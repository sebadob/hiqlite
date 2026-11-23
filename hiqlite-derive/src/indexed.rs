// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::from_row::{ColumnAttr, FieldSpec, RowPlan};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Attribute;
use syn::LitInt;
use syn::LitStr;
use syn::Type;
use syn::WhereClause;
use syn::parse::Parser;

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
///
/// A plan that flattens a field gets nothing from this function.  Such a type is never registered
/// (`from_row::register_marker`), so a query never runs a hoisted impl of it, and such an impl would
/// need the column list of every part it flattens: a `where` bound naming a type that is not a
/// generic parameter is checked where it is written, so it would fail to compile for exactly the
/// hand-written parts that make `flatten` useful.  The shape of a flattening type comes from
/// `shape_impl` below, which asks each part for its own shape.
pub(crate) fn impl_indexed(plan: &RowPlan, kind: IndexedKind) -> TokenStream {
    if plan.flattens() {
        return TokenStream::new();
    }
    let name = &plan.name;
    let (impl_generics, ty_generics, where_clause) = plan.generics.split_for_impl();
    let trait_path = kind.trait_path();
    let map_fn = kind.map_fn();

    // Every slot of a type without `flatten` is one of its own fields: `skip` reads no column and
    // takes no slot.
    let own_count = own_names(plan).len();

    let mut map_entries: Vec<(String, LitInt)> = Vec::new();
    let mut body = Vec::new();
    let mut slot: usize = 0;

    for field in &plan.fields {
        let id = &field.ident;
        let ts = match field.attr {
            ColumnAttr::Skip => quote! {
                #id: ::std::default::Default::default(),
            },
            ColumnAttr::Flatten => unreachable!("a plan that flattens a field has no hoisted impl"),
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
    let map_entries = map_entries
        .iter()
        .map(|(name, slot)| {
            let lit = LitStr::new(name, Span::call_site());
            quote! { (#lit, #slot), }
        })
        .collect::<Vec<_>>();

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

    // The shape `sql_map!()` compares a query's columns against is published by `shape_impl`, from
    // the `FromRow` side only, so this function emits the index impls of one family and nothing else.
    quote! {
        impl #impl_generics #trait_path for #name #ty_generics #where_clause {
            const FIELD_COUNT: usize = #own_count;

            fn field_maps() -> ::hiqlite::FieldMaps<'static> {
                const NAMES: &::hiqlite::FieldNames = &::hiqlite::FieldNames::new(&[
                    #(#map_entries)*
                ]);
                let maps: ::hiqlite::FieldMaps<'static> = ::std::vec![(NAMES, 0u16)];
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

/// The shape `sql_map!()` compares a query's columns against: the own column names of the plan, plus
/// every flattened part through its own `SqlShape`.
///
/// Each part is a `where` bound, and a bound naming a type that is not a generic parameter is
/// checked where it is written, so a part that publishes no shape is an error at the parent.  That is
/// the point: a `sql_map!()` of this type can only be checked against names the derive can see, and
/// the columns a hand-written `From<&mut Row>` reads are not among them.  A part gets its shape from
/// its own `FromRow` derive, from the `SqlShape` derive, or from a hand-written `SqlShape` impl.
///
/// Only the `FromRow` side publishes it: a type deriving both would get the same impl twice, and
/// `query_try_map` reports a column mismatch as an error at runtime, so the non-panicking side needs
/// none; a type mapped only through `TryFromRow` publishes its shape with the `SqlShape` derive.
///
/// It is emitted where a check can run, and nowhere else: a debug build always, a release build only
/// when it opts into `sql-check-release`.  A plain release build emits no `sql_map!()` comparison, so
/// a shape in it would be code that nothing can call.
pub(crate) fn shape_impl(plan: &RowPlan) -> TokenStream {
    let name = &plan.name;
    let (impl_generics, ty_generics, where_clause) = plan.generics.split_for_impl();
    let shape_names = own_names(plan)
        .into_iter()
        .map(|name| LitStr::new(&name, Span::call_site()));
    let parts = flatten_types(plan);
    // `wrapping_add`, exactly like `sql_shape` does inside: two FNV values of six columns each are
    // well past half of `u64`, so a nested type would overflow a plain `+` and a debug build would
    // stop the compile of a `sql_map!()` that has nothing wrong with it.
    let shape_parts = parts
        .iter()
        .map(|ty| {
            quote! { .wrapping_add(<#ty as ::hiqlite::SqlShape>::SHAPE) }
        })
        .collect::<Vec<_>>();
    let where_clause = with_bounds(
        where_clause.cloned(),
        &parts,
        &quote! { ::hiqlite::SqlShape },
    );
    // The one gate the shape needs: a debug build always compares, a release build only when it
    // opted into the check - so the gate is off exactly when the impl is needed in a release build.
    // The gate is applied by parsing the impl and attaching the attribute to it: a `cfg` attribute
    // takes the item it gates as its input, so it cannot be written in front of the `impl`.
    let gate = (!crate::shape::checks_run_in_release()).then(parse_cfg);
    let mut imp: syn::ItemImpl = syn::parse_quote! {
        impl #impl_generics ::hiqlite::SqlShape for #name #ty_generics #where_clause {
            const SHAPE: u64 = ::hiqlite::sql_shape(&[ #(#shape_names),* ]) #(#shape_parts)*;
        }
    };
    if let Some(cfg) = gate {
        imp.attrs.push(cfg);
    }
    quote! { #imp }
}

/// The one attribute the shape gate needs, parsed from its own tokens so it is written exactly as
/// the macro emits it. The macro itself gates the shape comparison of `sql_map!` the same way.
fn parse_cfg() -> Attribute {
    Attribute::parse_outer
        .parse2(quote! { #[cfg(debug_assertions)] })
        .expect("a cfg attribute is valid Rust")
        .remove(0)
}

/// The column names this plan reads itself: every field that is neither `skip` nor `flatten`, with
/// its `rename` applied, sorted by name.
fn own_names(plan: &RowPlan) -> Vec<String> {
    let mut names = plan
        .fields
        .iter()
        .filter(|f| !matches!(f.attr, ColumnAttr::Skip | ColumnAttr::Flatten))
        .map(|f| f.col_name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// The types of the `flatten` fields of a plan, in declaration order.
fn flatten_types(plan: &RowPlan) -> Vec<Type> {
    plan.fields
        .iter()
        .filter(|f| matches!(f.attr, ColumnAttr::Flatten))
        .map(|f| f.ty.clone())
        .collect()
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
