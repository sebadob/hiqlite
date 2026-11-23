// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::from_row::{ColumnAttr, FieldSpec, RowPlan};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{LitInt, LitStr, Type, WhereClause};

/// Which side of the hoisted index path to generate.
#[derive(Clone, Copy)]
pub(crate) enum IndexedKind {
    /// `FromRowIndexed`: reads via `get_idx`, a type mismatch panics.
    From,
    /// `TryFromRowIndexed`: reads via `try_get_idx`, a type mismatch is an `hiqlite::Error`.
    Try,
}

impl IndexedKind {
    fn trait_path(self) -> TokenStream {
        match self {
            Self::From => quote!(::hiqlite::FromRowIndexed),
            Self::Try => quote!(::hiqlite::TryFromRowIndexed),
        }
    }

    fn map_fn(self) -> TokenStream {
        match self {
            Self::From => quote!(from_row_indices),
            Self::Try => quote!(try_from_row_indices),
        }
    }

    fn get_fn(self) -> TokenStream {
        match self {
            Self::From => quote!(get_idx),
            Self::Try => quote!(try_get_idx),
        }
    }

    fn is_try(self) -> bool {
        matches!(self, Self::Try)
    }
}

/// Generates the hoisted implementation for a validated row plan.
///
/// The field names go into a sorted compile-time table; the query resolves them to column positions
/// once before the first row, and the mapping below only ever indexes into that result, so a row
/// does no name lookup at all.  Field order and column order stay independent, because the
/// resolution reads the column names of the result set and never assumes an order.
pub(crate) fn impl_indexed(plan: &RowPlan, kind: IndexedKind) -> TokenStream {
    let name = &plan.name;
    let (impl_generics, ty_generics, where_clause) = plan.generics.split_for_impl();
    let trait_path = kind.trait_path();
    let map_fn = kind.map_fn();

    let own_count = plan
        .fields
        .iter()
        .filter(|f| !matches!(f.attr, ColumnAttr::Skip | ColumnAttr::Flatten))
        .count();

    // The slot space of the type: the own fields first, then one segment per `flatten` field, in
    // declaration order.  `skip` reads no column and takes no slot.
    let mut base: TokenStream = usize_lit(own_count).to_token_stream();
    let mut bounds = Vec::new();

    let mut map_entries: Vec<(String, LitInt)> = Vec::new();
    let mut body = Vec::new();
    let mut slot: usize = 0;

    for field in &plan.fields {
        let id = &field.ident;
        let ts = match field.attr {
            ColumnAttr::Skip => quote! {
                #id: ::std::default::Default::default(),
            },
            ColumnAttr::Flatten => {
                let ty = &field.ty;
                bounds.push(field.ty.clone());
                let seg = quote! { &idx[#base..#base + <#ty as #trait_path>::FIELD_COUNT] };
                let call = quote! { <#ty as #trait_path>::#map_fn(row, #seg) };
                base = quote! { #base + <#ty as #trait_path>::FIELD_COUNT };
                if kind.is_try() {
                    quote! { #id: #call?, }
                } else {
                    quote! { #id: #call, }
                }
            }
            _ => {
                let local = usize_lit(slot);
                // The map stores the slot as a `u16`, the slice index as a `usize`.
                let value = LitInt::new(&format!("{slot}u16"), Span::call_site());
                slot += 1;
                map_entries.push((field.col_name.clone(), value));
                read_expr(field, kind, &local)
            }
        };
        body.push(ts);
    }

    // Sorted by name, which is what `FieldNames::get` binary searches against.  The order is
    // derived, so a field renamed in the middle of a struct moves within the table only.
    map_entries.sort_by(|a, b| a.0.cmp(&b.0));
    let shape_names = map_entries
        .iter()
        .map(|(name, _)| LitStr::new(name, Span::call_site()))
        .collect::<Vec<_>>();
    let map_entries = map_entries
        .iter()
        .map(|(name, slot)| {
            let lit = LitStr::new(name, Span::call_site());
            quote! { (#lit, #slot), }
        })
        .collect::<Vec<_>>();

    let field_count = base;

    let extend = extend_segments(&bounds, &trait_path, own_count);
    // The segments are built with `Vec::extend`, which needs `mut`; a type with only its own
    // fields is one `vec!` literal and needs none, so the generated code never carries a mut it
    // does not use.
    let maps_init = if bounds.is_empty() {
        quote! {
            let maps: ::hiqlite::FieldMaps<'static> = ::std::vec![(NAMES, 0u16)];
        }
    } else {
        quote! {
            let mut maps: ::hiqlite::FieldMaps<'static> = ::std::vec::Vec::new();
            maps.push((NAMES, 0u16));
            #(#extend)*
        }
    };
    let where_base = where_clause.cloned();
    let where_clause = with_bounds(where_base.clone(), &bounds, &trait_path);
    let shape_parts = bounds
        .iter()
        .map(|ty| quote! { + <#ty as ::hiqlite::SqlShape>::SHAPE })
        .collect::<Vec<_>>();
    let shape_where = with_bounds(where_base, &bounds, &quote! { ::hiqlite::SqlShape });
    let from_str_import = plan
        .with_from_str
        .then(|| quote! { use ::std::str::FromStr; });

    let (signature, tail) = if kind.is_try() {
        (
            quote! {
                fn #map_fn(
                    row: &mut ::hiqlite::Row<'_>,
                    idx: &[::hiqlite::ColIndex],
                ) -> ::std::result::Result<Self, ::hiqlite::Error>
            },
            quote! { ::std::result::Result::Ok(Self { #(#body)* }) },
        )
    } else {
        (
            quote! {
                fn #map_fn(row: &mut ::hiqlite::Row<'_>, idx: &[::hiqlite::ColIndex]) -> Self
            },
            quote! { Self { #(#body)* } },
        )
    };

    // The shape `sql_map!()` compares a query's columns against: the own names, plus every
    // flattened part through its own `SqlShape`.  Only the `FromRow` side publishes it - a type
    // deriving both derives would otherwise get the same impl twice - and `query_try_map` reports a
    // column mismatch as an error at runtime, which is why its side needs none.
    let shape = (!kind.is_try()).then(|| {
        quote! {
            impl #impl_generics ::hiqlite::SqlShape for #name #ty_generics #shape_where {
                const SHAPE: u64 = ::hiqlite::sql_shape(&[ #(#shape_names),* ]) #(#shape_parts)*;
            }
        }
    });

    quote! {
        #shape

        impl #impl_generics #trait_path for #name #ty_generics #where_clause {
            const FIELD_COUNT: usize = #field_count;

            fn field_maps() -> ::hiqlite::FieldMaps<'static> {
                const NAMES: &::hiqlite::FieldNames = &::hiqlite::FieldNames::new(&[
                    #(#map_entries)*
                ]);
                #maps_init
                maps
            }

            #[inline]
            #signature {
                #from_str_import
                #tail
            }
        }
    }
}

/// The read expression of one field, mirroring the name-based derive with index reads.
fn read_expr(field: &FieldSpec, kind: IndexedKind, local: &LitInt) -> TokenStream {
    let id = &field.ident;
    let col_name = &field.col_name;
    let idx = quote! { idx[#local] };
    let get_fn = kind.get_fn();
    let try_ = kind.is_try();

    match field.attr {
        ColumnAttr::Skip | ColumnAttr::Flatten => unreachable!("handled by the caller"),
        ColumnAttr::None => {
            let ty = &field.ty;
            if try_ {
                quote! { #id: row.#get_fn::<#ty>(#idx)?, }
            } else {
                quote! { #id: row.#get_fn::<#ty>(#idx), }
            }
        }
        ColumnAttr::FromI32 => {
            let msg = format!("column '{col_name}' does not fit into i32");
            let convert = quote! { <i32 as ::std::convert::TryFrom<i64>>::try_from(i) };
            if field.is_opt {
                if try_ {
                    quote! {
                        #id: row.#get_fn::<Option<i64>>(#idx)?
                            .map(|i| #convert)
                            .transpose()
                            .map_err(|_| ::hiqlite::Error::Sqlite(#msg.into()))?
                            .map(|i| i.into()),
                    }
                } else {
                    quote! {
                        #id: row.#get_fn::<Option<i64>>(#idx)
                            .map(|i| #convert.expect(#msg).into()),
                    }
                }
            } else if try_ {
                quote! {
                    #id: {
                        let i = row.#get_fn::<i64>(#idx)?;
                        #convert
                            .map_err(|_| ::hiqlite::Error::Sqlite(#msg.into()))?
                            .into()
                    },
                }
            } else {
                quote! {
                    #id: {
                        let i = row.#get_fn::<i64>(#idx);
                        #convert.expect(#msg).into()
                    },
                }
            }
        }
        ColumnAttr::FromI64 => {
            if field.is_opt {
                if try_ {
                    quote! { #id: row.#get_fn::<Option<i64>>(#idx)?.map(|i| i.into()), }
                } else {
                    quote! { #id: row.#get_fn::<Option<i64>>(#idx).map(|i| i.into()), }
                }
            } else if try_ {
                quote! { #id: row.#get_fn::<i64>(#idx)?.into(), }
            } else {
                quote! { #id: row.#get_fn::<i64>(#idx).into(), }
            }
        }
        ColumnAttr::Parse => {
            let msg = format!("failed to parse column '{col_name}'");
            if field.is_opt {
                if try_ {
                    quote! {
                        #id: row.#get_fn::<Option<String>>(#idx)?
                            .map(|s| s.parse().map_err(|_| ::hiqlite::Error::Sqlite(#msg.into())))
                            .transpose()?,
                    }
                } else {
                    quote! {
                        #id: row.#get_fn::<Option<String>>(#idx)
                            .map(|s| s.parse().expect(#msg)),
                    }
                }
            } else if try_ {
                quote! {
                    #id: row.#get_fn::<String>(#idx)?
                        .parse()
                        .map_err(|_| ::hiqlite::Error::Sqlite(#msg.into()))?,
                }
            } else {
                quote! {
                    #id: row.#get_fn::<String>(#idx).parse().expect(#msg),
                }
            }
        }
        ColumnAttr::FromString => {
            if field.is_opt {
                if try_ {
                    quote! { #id: row.#get_fn::<Option<String>>(#idx)?.map(|s| s.into()), }
                } else {
                    quote! { #id: row.#get_fn::<Option<String>>(#idx).map(|s| s.into()), }
                }
            } else if try_ {
                quote! { #id: row.#get_fn::<String>(#idx)?.into(), }
            } else {
                quote! { #id: row.#get_fn::<String>(#idx).into(), }
            }
        }
    }
}

/// One `maps.extend(...)` per flattened field, shifting the segment offsets of the nested type by
/// the slots that come before it.
fn extend_segments(
    nested: &[Type],
    trait_path: &TokenStream,
    own_count: usize,
) -> Vec<TokenStream> {
    let mut base: TokenStream = usize_lit(own_count).to_token_stream();
    let mut out = Vec::new();
    for ty in nested {
        out.push(quote! {
            maps.extend(
                <#ty as #trait_path>::field_maps()
                    .into_iter()
                    .map(|(m, o)| (m, o + (#base) as u16)),
            );
        });
        base = quote! { #base + <#ty as #trait_path>::FIELD_COUNT };
    }
    out
}

/// Adds a `#ty: Trait` bound for every flattened field to the where clause of the impl.
pub(crate) fn with_bounds(
    clause: Option<WhereClause>,
    nested: &[Type],
    trait_path: &TokenStream,
) -> Option<WhereClause> {
    let mut clause = match (clause, nested.is_empty()) {
        (clause, true) => return clause,
        (Some(clause), false) => clause,
        (None, false) => syn::parse_quote!(where),
    };
    for ty in nested {
        clause
            .predicates
            .push(syn::parse_quote! { #ty: #trait_path });
    }
    Some(clause)
}

fn usize_lit(v: usize) -> LitInt {
    LitInt::new(&format!("{v}usize"), Span::call_site())
}
