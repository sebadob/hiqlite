// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, GenericArgument, GenericParam, Generics, Ident, LitStr, Meta,
    MetaList, PathArguments, Type,
};

/// The validated derive input shared by the `FromRow` and `TryFromRow` derives.
pub(crate) struct RowPlan {
    pub name: Ident,
    /// Owned copy of the input generics; each derive re-splits it for codegen.
    pub generics: Generics,
    /// Whether any field uses `parse`, so the generated code needs `FromStr` in scope.
    pub with_from_str: bool,
    pub fields: Vec<FieldSpec>,
}

/// A single resolved struct field.
pub(crate) struct FieldSpec {
    pub ident: Ident,
    /// The field's declared type, for plain (attribute-less) column reads.
    pub ty: Type,
    pub attr: ColumnAttr,
    /// Resolved column name: the `rename` if present, else the field name.
    pub col_name: String,
    pub rename: Option<LitStr>,
    pub is_opt: bool,
}

/// Validates the derive input and resolves every field to its column attribute.
///
/// Shared by `FromRow` (panicking) and `TryFromRow` (non-panicking), so both enforce the same
/// rules with the same error messages.
pub(crate) fn plan_fields(input: DeriveInput) -> syn::Result<RowPlan> {
    let name = input.ident;
    let generics = input.generics.clone();

    let Data::Struct(data) = input.data else {
        return Err(syn::Error::new(
            name.span(),
            "FromRow can only be derived for a `struct`",
        ));
    };

    let mut with_from_str = false;
    // (column name, field name) of every field that reads a column, for duplicate detection.
    let mut columns: Vec<(String, String)> = Vec::new();
    let mut fields = Vec::new();

    for field in &data.fields {
        let Some(id) = &field.ident else {
            return Err(syn::Error::new(
                field.ty.span(),
                "FromRow only supports structs with named fields; tuple fields are not supported",
            ));
        };

        let ch = ColumnHandler::try_from(field.attrs.as_slice())?;
        let is_opt = is_field_ty_opt(&field.ty).unwrap_or(false);
        let col_name = ch
            .rename
            .as_ref()
            .map_or_else(|| id.to_string(), |r| r.value());

        // skip and flatten do not read a single named column, so they cannot collide.
        if !matches!(ch.attr, ColumnAttr::Skip | ColumnAttr::Flatten) {
            if let Some((_, other_field)) = columns.iter().find(|(c, _)| *c == col_name) {
                return Err(syn::Error::new(
                    field.span(),
                    format!(
                        "duplicate column '{col_name}': fields `{other_field}` and `{id}` both map \
                         to it, but a column can only be read once"
                    ),
                ));
            }
            columns.push((col_name.clone(), id.to_string()));
        }

        match ch.attr {
            ColumnAttr::FromI32 => check_unsigned_target(&field.ty, is_opt, "i32")?,
            ColumnAttr::FromI64 => check_unsigned_target(&field.ty, is_opt, "i64")?,
            _ => {}
        }

        if matches!(ch.attr, ColumnAttr::Parse) {
            with_from_str = true;
        }

        fields.push(FieldSpec {
            ident: id.clone(),
            ty: field.ty.clone(),
            attr: ch.attr,
            col_name,
            rename: ch.rename,
            is_opt,
        });
    }

    Ok(RowPlan {
        name,
        generics,
        with_from_str,
        fields,
    })
}

pub fn impl_from_row(input: DeriveInput) -> syn::Result<TokenStream> {
    let plan = plan_fields(input)?;
    let name = &plan.name;
    let (impl_generics, ty_generics, where_clause) = plan.generics.split_for_impl();

    let mut body = Vec::new();

    for field in &plan.fields {
        let id = &field.ident;
        let col_name = &field.col_name;
        let name = if let Some(rename) = &field.rename {
            rename.to_token_stream()
        } else {
            quote! {#col_name}
        };

        let ts = match field.attr {
            ColumnAttr::None => {
                let ty = &field.ty;
                quote! {
                    #id: row.get::<#ty>(#name),
                }
            }
            ColumnAttr::Flatten => quote! {
                #id: ::std::convert::TryFrom::try_from(&mut *row).expect("failed to flatten column"),
            },
            ColumnAttr::FromI32 => {
                let msg = format!("column '{col_name}' does not fit into i32");
                let convert = quote! {
                    <i32 as ::std::convert::TryFrom<i64>>::try_from(i).expect(#msg).into()
                };
                if field.is_opt {
                    quote! {
                        #id: row.get::<Option<i64>>(#name)
                            .map(|i| #convert),
                    }
                } else {
                    quote! {
                        #id: {
                            let i = row.get::<i64>(#name);
                            #convert
                        },
                    }
                }
            }
            ColumnAttr::FromI64 => {
                if field.is_opt {
                    quote! {
                        #id: row.get::<Option<i64>>(#name).map(|i| i.into()),
                    }
                } else {
                    quote! {
                        #id: row.get::<i64>(#name).into(),
                    }
                }
            }
            ColumnAttr::Parse => {
                let msg = format!("failed to parse column '{col_name}'");
                if field.is_opt {
                    quote! {
                        #id: row.get::<Option<String>>(#name)
                            .map(|s| s.parse().expect(#msg)),
                    }
                } else {
                    quote! {
                        #id: row.get::<String>(#name).parse().expect(#msg),
                    }
                }
            }
            ColumnAttr::FromString => {
                if field.is_opt {
                    quote! {
                        #id: row.get::<Option<String>>(#name).map(|s| s.into()),
                    }
                } else {
                    quote! {
                        #id: row.get::<String>(#name).into(),
                    }
                }
            }
            ColumnAttr::Skip => quote! {
                #id: ::std::default::Default::default(),
            },
        };

        body.push(ts);
    }

    let from_str_import = if plan.with_from_str {
        Some(quote! {
            use ::std::str::FromStr;
        })
    } else {
        None
    };

    let indexed_from = crate::indexed::impl_indexed(&plan, crate::indexed::IndexedKind::From);
    let indexed_try = crate::indexed::impl_indexed(&plan, crate::indexed::IndexedKind::Try);

    // The name-based `From` body is the first code of a type that any query runs, so it is the
    // place where the type hands its hoisted mapping to the query engine, once per type.  The
    // bounds a hoisted impl needs over its `flatten` fields belong to that impl alone: the `From`
    // impl keeps exactly the bounds it always had, so a type that flattens a hand-written
    // `From<&mut Row>` keeps compiling and stays on the name-based path.
    let register = register_marker(
        &plan,
        &quote! { ::hiqlite::register_hoisted::<#name #ty_generics>() },
    );

    Ok(quote! {
        impl #impl_generics ::std::convert::From<&mut ::hiqlite::Row<'_>> for #name #ty_generics #where_clause {
            #[inline]
            fn from(row: &mut ::hiqlite::Row<'_>) -> Self {
                #from_str_import
                #register
                Self { #(#body)* }
            }
        }

        #indexed_from
        #indexed_try
    })
}

/// The registration of a type in the query engine's registry of hoisted mappings, fired on the
/// first row the type maps.
///
/// A type with a lifetime parameter is never `'static`, so it cannot enter the registry: such a
/// type is mapped by the names its body reads, which is the path it has always taken.
///
/// A type with a `flatten` field is not registered either.  Its hoisted impl exists only under the
/// bounds of its parts, which a `From` body cannot assume - a flattened type only has to provide a
/// `From<&mut Row>` - and registration must be something the body can always run.  Such a type is
/// mapped by its names, and a flat type with no `flatten` field gets the hoisted path.
pub(crate) fn register_marker(plan: &RowPlan, call: &TokenStream) -> Option<TokenStream> {
    let blocked = plan
        .generics
        .params
        .iter()
        .any(|p| matches!(p, GenericParam::Lifetime(_)))
        || plan
            .fields
            .iter()
            .any(|f| matches!(f.attr, ColumnAttr::Flatten));
    if blocked {
        return None;
    }
    Some(quote! {
        static REGISTERED: ::std::sync::atomic::AtomicBool =
            ::std::sync::atomic::AtomicBool::new(false);
        if !REGISTERED.load(::std::sync::atomic::Ordering::Relaxed) {
            REGISTERED.store(true, ::std::sync::atomic::Ordering::Relaxed);
            #call;
        }
    })
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ColumnAttr {
    None,
    Skip,
    Flatten,
    Parse,
    FromString,
    FromI64,
    FromI32,
}

struct ColumnHandler {
    attr: ColumnAttr,
    rename: Option<LitStr>,
}

impl TryFrom<&[Attribute]> for ColumnHandler {
    type Error = syn::Error;

    fn try_from(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut attr = ColumnAttr::None;
        let mut rename: Option<LitStr> = None;
        let mut column_span = None;

        for a in attrs {
            if !a.path().is_ident("column") {
                continue;
            }

            if column_span.is_some() {
                return Err(syn::Error::new(
                    a.span(),
                    "only one `#[column]` attribute per field is allowed; combine all options \
                     in a single attribute",
                ));
            }
            column_span = Some(a.span());

            let args = parse_column_list(a)?;
            attr = args.keyword.unwrap_or(ColumnAttr::None);
            rename = args.rename;
        }

        if let Some(span) = column_span
            && attr == ColumnAttr::None
            && rename.is_none()
        {
            return Err(syn::Error::new(
                span,
                "expected at least one column attribute, e.g. `rename = \"...\"` or a \
                 conversion keyword",
            ));
        }

        if matches!(attr, ColumnAttr::Skip | ColumnAttr::Flatten)
            && let Some(rename) = rename.as_ref()
        {
            return Err(syn::Error::new(
                rename.span(),
                "`skip` and `flatten` cannot be combined with `rename`, the column is never read",
            ));
        }

        Ok(Self { attr, rename })
    }
}

/// Parses a `#[column(...)]` attribute as a strict comma-separated list.
///
/// Supported entries: one conversion keyword (`skip`, `flatten`, `parse`, `from_string`,
/// `from_i64`, `from_i32`) and/or `rename = "..."` in either order. A trailing comma is
/// allowed; the whole stream must be consumed, so anything else is rejected.
struct ColumnArgs {
    keyword: Option<ColumnAttr>,
    rename: Option<LitStr>,
}

fn parse_column_list(attr: &Attribute) -> syn::Result<ColumnArgs> {
    let Meta::List(MetaList { path, tokens, .. }) = &attr.meta else {
        return Err(syn::Error::new(
            attr.span(),
            "expected `#[column(...)]` with a comma-separated list of attributes",
        ));
    };

    if !path.is_ident("column") {
        return Err(syn::Error::new(
            path.span(),
            "unexpected attribute, expected `column`",
        ));
    }

    let mut keyword: Option<ColumnAttr> = None;
    let mut rename: Option<LitStr> = None;
    let mut tokens: Vec<TokenTree> = tokens.clone().into_iter().collect();

    while let Some(first) = tokens.first().cloned() {
        match first {
            TokenTree::Punct(p) if p.as_char() == ',' => {
                // Comma separator or trailing comma.
                tokens.remove(0);
            }
            TokenTree::Ident(ident) if ident == "rename" && rename.is_none() => {
                tokens.remove(0);
                let Some(eq) = tokens.first().cloned() else {
                    return Err(syn::Error::new(ident.span(), "expected `=` after `rename`"));
                };
                if !matches!(&eq, TokenTree::Punct(p) if p.as_char() == '=') {
                    return Err(syn::Error::new(eq.span(), "expected `=` after `rename`"));
                }
                tokens.remove(0);
                let Some(lit) = tokens.first().cloned() else {
                    return Err(syn::Error::new(
                        ident.span(),
                        "expected a string literal after `rename =`",
                    ));
                };
                match lit {
                    TokenTree::Literal(l) => match syn::parse2::<LitStr>(l.to_token_stream()) {
                        Ok(s) => rename = Some(s),
                        Err(_) => {
                            return Err(syn::Error::new(
                                l.span(),
                                "`rename` expects a string literal, e.g. \
                                 `rename = \"my_column\"`",
                            ));
                        }
                    },
                    other => {
                        return Err(syn::Error::new(
                            other.span(),
                            "`rename` expects a string literal, e.g. \
                             `rename = \"my_column\"`",
                        ));
                    }
                }
                tokens.remove(0);
            }
            TokenTree::Ident(ident) if ident == "rename" => {
                return Err(syn::Error::new(
                    ident.span(),
                    "`rename` may only be used once per field",
                ));
            }
            TokenTree::Ident(ident) if keyword.is_none() => {
                tokens.remove(0);
                let kw = ident.to_string();
                keyword = Some(match kw.as_str() {
                    "skip" => ColumnAttr::Skip,
                    "flatten" => ColumnAttr::Flatten,
                    "parse" => ColumnAttr::Parse,
                    "from_string" => ColumnAttr::FromString,
                    "from_i64" => ColumnAttr::FromI64,
                    "from_i32" => ColumnAttr::FromI32,
                    other => {
                        return Err(syn::Error::new(
                            ident.span(),
                            format!("unknown column attribute '{other}'"),
                        ));
                    }
                });
            }
            other => {
                let found = other.to_string();
                let is_keyword = matches!(&other, TokenTree::Ident(i) if [
                    "skip", "flatten", "parse", "from_string", "from_i64", "from_i32"
                ]
                .contains(&i.to_string().as_str()));
                let message = if keyword.is_some() && is_keyword {
                    "only one conversion attribute per field is allowed; combine `rename` \
                     with it instead"
                        .to_string()
                } else if keyword.is_some() || rename.is_some() {
                    format!("expected `,` after column attribute, found `{found}`")
                } else {
                    format!(
                        "expected a column attribute keyword or `rename = \"...\"`, \
                         found `{found}`"
                    )
                };
                return Err(syn::Error::new(other.span(), message));
            }
        }
    }

    Ok(ColumnArgs { keyword, rename })
}

fn check_unsigned_target(ty: &Type, is_opt: bool, source: &str) -> syn::Result<()> {
    let ty = if is_opt { option_inner(ty)? } else { ty };

    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        let name = segment.ident.to_string();
        if ["u8", "u16", "u32", "u64", "usize"].contains(&name.as_str()) {
            return Err(syn::Error::new(
                ty.span(),
                format!(
                    "`{source}` cannot convert to the unsigned target `{name}` without a \
                     possible lossy cast. Enable the `cast_ints` feature and drop the \
                     attribute, or use `parse` instead."
                ),
            ));
        }
    }

    Ok(())
}

fn option_inner(ty: &Type) -> syn::Result<&Type> {
    let Type::Path(type_path) = ty else {
        return Err(syn::Error::new(
            ty.span(),
            "expected `Option<T>` for an optional field",
        ));
    };

    if type_path.path.segments.len() != 1 || type_path.path.segments[0].ident != "Option" {
        return Err(syn::Error::new(
            ty.span(),
            "expected `Option<T>` for an optional field",
        ));
    }

    let PathArguments::AngleBracketed(args) = &type_path.path.segments[0].arguments else {
        return Err(syn::Error::new(
            ty.span(),
            "expected `Option<T>` for an optional field",
        ));
    };

    match args.args.first() {
        Some(GenericArgument::Type(inner)) => Ok(inner),
        _ => Err(syn::Error::new(
            ty.span(),
            "expected `Option<T>` for an optional field",
        )),
    }
}

#[inline]
fn is_field_ty_opt(ty: &Type) -> Option<bool> {
    let Type::Path(ty) = ty else {
        return Some(false);
    };
    let mut iter = ty.path.segments.iter();

    let mut s = iter.next()?.ident.to_string();
    if s == "std" || s == "core" {
        s = iter.next()?.ident.to_string();
    }
    if s == "option" {
        s = iter.next()?.ident.to_string();
    }
    Some(s.as_str() == "Option")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn derive(code: &str) -> String {
        let input: DeriveInput = syn::parse_str(code).unwrap();
        impl_from_row(input).unwrap().to_string()
    }

    fn parse_error(code: &str) -> String {
        let input: DeriveInput = syn::parse_str(code).unwrap();
        match impl_from_row(input) {
            Ok(_) => panic!("the derive must not accept this input"),
            Err(e) => e.to_string(),
        }
    }

    /// Compact form of the generated tokens: `quote!` renders with spaces around `::`, `&` and `<`.
    fn compact(code: &str) -> String {
        code.replace(' ', "")
    }

    #[test]
    fn generates_the_from_impl() {
        let out = derive("struct Row { id: i64 }");
        assert!(
            compact(&out).contains("impl::std::convert::From<&mut::hiqlite::Row<'_>>forRow"),
            "missing From impl: {out}"
        );
        assert!(
            compact(&out).contains("fnfrom(row:&mut::hiqlite::Row<'_>)->Self"),
            "missing the `from` signature: {out}"
        );
    }

    #[test]
    fn reads_every_field_from_the_row() {
        let out = derive("struct Row { id: i64, name: String, opt: Option<i64> }");
        assert!(
            compact(&out).contains("Self{"),
            "missing struct literal: {out}"
        );
        for (field, ty) in [("id", "i64"), ("name", "String"), ("opt", "Option<i64>")] {
            assert!(
                compact(&out).contains(&format!("row.get::<{ty}>(\"{field}\")")),
                "field `{field}` is not read by its name with its type: {out}"
            );
        }
    }

    #[test]
    fn rename_uses_the_given_column_name() {
        let out = derive("struct Row { #[column(rename = \"the_id\")] id: i64 }");
        assert!(
            compact(&out).contains("row.get::<i64>(\"the_id\")"),
            "rename is not used as the column name: {out}"
        );
    }

    #[test]
    fn conversion_keywords_read_the_documented_types() {
        let out = derive("struct Row { #[column(from_i64)] big: i128 }");
        assert!(
            compact(&out).contains("row.get::<i64>(\"big\")") && out.contains(". into ()"),
            "from_i64 must read an i64 and convert it: {out}"
        );

        let out = derive("struct Row { #[column(from_i32)] num: i32 }");
        assert!(
            out.contains("TryFrom < i64 >") && out.contains(". expect ("),
            "from_i32 must narrow through TryFrom with a message: {out}"
        );

        let out = derive("struct Row { #[column(from_string)] name: String }");
        assert!(
            compact(&out).contains("row.get::<String>(\"name\")") && out.contains(". into ()"),
            "from_string must read a String and convert: {out}"
        );

        let out = derive("struct Row { #[column(parse)] date: chrono::NaiveDate }");
        assert!(
            compact(&out).contains("row.get::<String>(\"date\")")
                && out.contains(". parse ()")
                && compact(&out).contains("use::std::str::FromStr"),
            "parse must read text, import FromStr and parse: {out}"
        );
    }

    #[test]
    fn skip_leaves_the_field_to_default() {
        let out = derive("struct Row { id: i64, #[column(skip)] skipped: u32 }");
        assert!(
            compact(&out).contains("Default::default()"),
            "skip must default the field: {out}"
        );
        assert!(
            !compact(&out).contains("row.get(\"skipped\")"),
            "skip must not read a column: {out}"
        );
    }

    #[test]
    fn flatten_delegates_to_the_nested_conversion() {
        let out = derive("struct Row { #[column(flatten)] inner: Inner }");
        assert!(
            out.contains("TryFrom :: try_from (& mut * row)")
                && out.contains("failed to flatten column"),
            "flatten must delegate to the nested TryFrom: {out}"
        );
    }

    #[test]
    fn a_second_column_attribute_on_one_field_is_rejected() {
        let msg = parse_error("struct Row { #[column(rename = \"a\")] #[column(skip)] id: i64 }");
        assert!(
            msg.contains("only one `#[column]` attribute per field is allowed"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn a_bare_column_attribute_is_rejected() {
        let msg = parse_error("struct Row { #[column] id: i64 }");
        assert!(
            msg.contains("expected `#[column(...)]`"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn skip_and_rename_cannot_be_combined() {
        let msg = parse_error("struct Row { #[column(skip, rename = \"a\")] id: i64 }");
        assert!(
            msg.contains("cannot be combined with `rename`"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn an_unknown_column_keyword_is_rejected() {
        let msg = parse_error("struct Row { #[column(nope)] id: i64 }");
        assert!(!msg.is_empty(), "the derive must reject unknown keywords");
    }

    /// The check behind this test is the invariant the hoisted slot table and the `sql_map!()`
    /// shape are built on: a type never reads the same column twice.
    #[test]
    fn two_fields_mapping_to_one_column_are_rejected() {
        let msg = parse_error(
            "struct Row { #[column(rename = \"x\")] a: i64, b: i64, #[column(rename = \"x\")] \
             c: String }",
        );
        assert!(
            msg.contains("duplicate column 'x'"),
            "fields `a` and `c` both map to `x`: {msg}"
        );
    }

    #[test]
    fn a_tuple_struct_is_rejected() {
        let msg = parse_error("struct Row(i64);");
        assert!(
            msg.contains("named fields"),
            "a tuple field has no name to map: {msg}"
        );
    }

    #[test]
    fn an_enum_is_rejected() {
        let msg = parse_error("enum Row { A }");
        assert!(
            msg.contains("derived for a `struct`"),
            "the derive maps the fields of a struct: {msg}"
        );
    }

    #[test]
    fn an_unsigned_target_of_a_conversion_is_rejected() {
        let msg = parse_error("struct Row { #[column(from_i32)] a: u8 }");
        assert!(
            msg.contains("cast_ints"),
            "the lossy cast must name the feature that allows it: {msg}"
        );
    }

    #[test]
    fn generates_the_hoisted_impls() {
        let out = derive("struct Row { id: i64, name: String }");
        let c = compact(&out);
        assert!(
            c.contains("impl::hiqlite::FromRowIndexedforRow"),
            "missing FromRowIndexed impl: {out}"
        );
        assert!(
            c.contains("impl::hiqlite::TryFromRowIndexedforRow"),
            "missing TryFromRowIndexed impl: {out}"
        );
        assert!(
            c.contains("constFIELD_COUNT:usize=2usize"),
            "field count wrong: {out}"
        );
        assert!(
            c.contains(r#"=&::hiqlite::FieldNames::new(&[("id",0u16),("name",1u16),])"#),
            "the sorted field table is missing: {out}"
        );
        assert!(
            c.contains("row.get_idx::<i64>(idx[0usize])")
                && c.contains("row.get_idx::<String>(idx[1usize])"),
            "fields not read by slot index: {out}"
        );
        assert!(
            c.contains("row.try_get_idx::<i64>(idx[0usize])?"),
            "the Try impl must not read by name: {out}"
        );
    }

    #[test]
    fn a_flattened_field_gets_its_own_segment() {
        let out = derive("struct Row { id: i64, #[column(flatten)] inner: Inner }");
        let c = compact(&out);
        assert!(
            c.contains(
                "constFIELD_COUNT:usize=1usize+<Inneras::hiqlite::FromRowIndexed>::FIELD_COUNT"
            ),
            "nested fields not counted: {out}"
        );
        assert!(
            c.contains("maps.extend(<Inneras::hiqlite::FromRowIndexed>::field_maps()"),
            "nested segment not merged: {out}"
        );
        assert!(
            c.contains("o+(1usize)asu16"),
            "nested segment not offset behind the own fields: {out}"
        );
        assert!(
            c.contains("&idx[1usize..1usize+<Inneras::hiqlite::FromRowIndexed>::FIELD_COUNT]"),
            "nested slice starts at the wrong slot: {out}"
        );
        assert!(
            c.contains("whereInner:::hiqlite::FromRowIndexed"),
            "missing the bound of the nested type: {out}"
        );
    }

    #[test]
    fn two_flattened_fields_are_offset_behind_each_other() {
        let out =
            derive("struct Row { #[column(flatten)] a: A, id: i64, #[column(flatten)] b: B }");
        let c = compact(&out);
        assert!(
            c.contains("o+(1usize)asu16")
                && c.contains("o+(1usize+<Aas::hiqlite::FromRowIndexed>::FIELD_COUNT)asu16"),
            "segments not chained: {out}"
        );
    }

    #[test]
    fn the_from_impl_registers_the_hoisted_mapping() {
        let out = derive("struct Row { id: i64 }");
        let c = compact(&out);
        assert!(
            c.contains("::hiqlite::register_hoisted::<Row>()"),
            "the From body must register the type: {out}"
        );
        assert!(
            c.contains(
                "staticREGISTERED:::std::sync::atomic::AtomicBool=::std::sync::atomic::AtomicBool::new(false)"
            ),
            "registration must run once per type: {out}"
        );
    }

    #[test]
    fn a_struct_with_a_lifetime_is_never_registered() {
        let out = derive("struct Row<'a> { id: &'a i64 }");
        let c = compact(&out);
        assert!(
            !c.contains("register_hoisted"),
            "a type that is not 'static cannot enter the registry: {out}"
        );
    }

    /// Registration is blocked for the same reason the `From` body is kept free of hoisted bounds:
    /// a flattened type only has to provide a `From<&mut Row>`, so the parent cannot register a
    /// mapping that needs its nested type to be hoisted as well.
    #[test]
    fn a_struct_with_a_flattened_field_is_never_registered() {
        let out = derive("struct Row { id: i64, #[column(flatten)] inner: Inner }");
        let c = compact(&out);
        assert!(
            !c.contains("register_hoisted"),
            "a flattened field keeps the type on the name-based path: {out}"
        );
    }

    #[test]
    fn the_shape_of_a_mapped_type_is_published_once() {
        let out = compact(&derive("struct Row { id: i64, name: String }"));
        assert!(
            out.contains("impl::hiqlite::SqlShapeforRow{constSHAPE:u64=::hiqlite::sql_shape(")
                && out.contains(r#"&["id","name"]);"#),
            "the shape `sql_map!()` compares a query's columns against is missing: {out}"
        );
        assert_eq!(
            out.matches("impl::hiqlite::SqlShapeforRow").count(),
            1,
            "the derive emits both mapping sides and the shape exactly once: {out}"
        );

        let flat = compact(&derive(
            "struct Row { id: i64, #[column(flatten)] inner: Inner }",
        ));
        assert!(
            flat.contains("+<Inneras::hiqlite::SqlShape>::SHAPE"),
            "a flattened part must contribute its own shape: {flat}"
        );
    }

    #[test]
    fn only_a_flattened_type_needs_the_mutable_segment_list() {
        let plain = compact(&derive("struct Row { id: i64 }"));
        assert!(
            plain.contains("letmaps:::hiqlite::FieldMaps<\'static>=::std::vec![(NAMES,0u16)]"),
            "own fields only must build the segments in one literal: {plain}"
        );
        assert!(
            !plain.contains("letmutmaps"),
            "no `mut` without segments to extend: {plain}"
        );

        let flat = compact(&derive("struct Row { #[column(flatten)] inner: Inner }"));
        assert!(
            flat.contains("letmutmaps:::hiqlite::FieldMaps<\'static>=::std::vec::Vec::new()")
                && flat.contains("maps.push((NAMES,0u16))")
                && flat.contains("maps.extend("),
            "segments of a nested type are pushed and extended: {flat}"
        );
    }

    #[test]
    fn a_skipped_field_takes_no_slot() {
        let out = derive("struct Row { id: i64, #[column(skip)] extra: u32 }");
        let c = compact(&out);
        assert!(
            c.contains("constFIELD_COUNT:usize=1usize"),
            "skip counted as a column: {out}"
        );
        assert!(
            !c.contains(r#"("extra","#),
            "skip must not appear in the field map: {out}"
        );
    }

    #[test]
    fn a_derived_generic_struct_keeps_its_generics() {
        let out = derive("struct Wrapper<T> { inner: T, id: i64 }");
        assert!(
            compact(&out)
                .contains("impl<T>::std::convert::From<&mut::hiqlite::Row<'_>>forWrapper<T>"),
            "generic impl header: {out}"
        );
    }
}
