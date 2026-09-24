// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, GenericArgument, Generics, Ident, LitStr, Meta, MetaList,
    PathArguments, Type,
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

    for field in data.fields.iter() {
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
            .map(|r| r.value())
            .unwrap_or_else(|| id.to_string());

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

    Ok(quote! {
        impl #impl_generics ::std::convert::From<&mut ::hiqlite::Row<'_>> for #name #ty_generics #where_clause {
            #[inline]
            fn from(row: &mut ::hiqlite::Row<'_>) -> Self {
                #from_str_import
                Self { #(#body)* }
            }
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

        for a in attrs.iter() {
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

fn is_field_ty_opt(ty: &Type) -> syn::Result<bool> {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            if path.segments.len() == 1 && path.segments[0].ident == "Option" {
                Ok(true)
            } else {
                Ok(false)
            }
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    fn generate(input: &str) -> String {
        let input = parse_str::<DeriveInput>(input).unwrap();
        impl_from_row(input).unwrap().to_string()
    }

    fn generate_err(input: &str) -> String {
        let input = parse_str::<DeriveInput>(input).unwrap();
        match impl_from_row(input) {
            Ok(ts) => panic!("expected error, got: {ts}"),
            Err(err) => err.to_compile_error().to_string(),
        }
    }

    #[test]
    fn basic_mapping_uses_row_get_by_column_name() {
        let out = generate(
            r#"struct Test { #[column(rename = "name_db")] name: String, skip_me: bool }"#,
        );
        // `quote!` renders with spaces around `::`, so shape checks run on the compact form.
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("From<&mut::hiqlite::Row"),
            "missing From impl: {out}"
        );
        assert!(out.contains("name_db"), "rename not honored: {out}");
    }

    #[test]
    fn from_i32_uses_try_from_and_never_clamps() {
        let out = generate(
            r#"struct Test { #[column(from_i32)] a: i32, #[column(from_i32)] b: Option<i32> }"#,
        );
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("TryFrom<i64>>::try_from"),
            "missing try_from conversion: {out}"
        );
        assert!(
            !compact.contains("cmp::min"),
            "silent clamp still present: {out}"
        );
        assert!(
            !compact.contains("cmp::max"),
            "silent clamp still present: {out}"
        );
        assert!(
            out.contains("does not fit into i32"),
            "no panic message: {out}"
        );
    }

    #[test]
    fn trailing_comma_is_accepted() {
        let out =
            generate(r#"struct Test { #[column(from_i32,)] a: i16, #[column(skip,)] b: bool }"#);
        let compact = out.replace(' ', "");
        assert!(compact.contains("TryFrom<i64>>::try_from"), "got: {out}");
    }

    #[test]
    fn trailing_junk_is_rejected() {
        let err = generate_err(r#"struct Test { #[column(from_i32 junk)] a: i16 }"#);
        assert!(err.contains("expected `,`"), "got: {err}");
    }

    #[test]
    fn rename_requires_string_literal() {
        let err = generate_err(r#"struct Test { #[column(rename = 123)] a: i64 }"#);
        assert!(err.contains("string literal"), "got: {err}");
    }

    #[test]
    fn unknown_keyword_is_rejected() {
        let err = generate_err(r#"struct Test { #[column(from_i16)] a: i16 }"#);
        assert!(
            err.contains("unknown column attribute 'from_i16'"),
            "got: {err}"
        );
    }

    #[test]
    fn empty_attribute_is_rejected() {
        let err = generate_err(r#"struct Test { #[column()] a: i64 }"#);
        assert!(
            err.contains("expected at least one column attribute"),
            "got: {err}"
        );
    }

    #[test]
    fn multiple_column_attrs_are_rejected() {
        let err =
            generate_err(r#"struct Test { #[column(from_i32)] #[column(rename = "a")] a: i16 }"#);
        assert!(err.contains("only one `#[column]`"), "got: {err}");
    }

    #[test]
    fn skip_cannot_be_combined_with_rename() {
        let err = generate_err(r#"struct Test { #[column(skip, rename = "x")] a: i64 }"#);
        assert!(err.contains("cannot be combined"), "got: {err}");
    }

    #[test]
    fn duplicate_columns_are_rejected() {
        let err = generate_err(
            r#"struct Test { #[column(rename = "x")] a: i64, b: i64, #[column(rename = "x")] c: String }"#,
        );
        assert!(err.contains("duplicate column 'x'"), "got: {err}");
    }

    #[test]
    fn tuple_struct_is_rejected() {
        let err = generate_err(r#"struct Test(i64);"#);
        assert!(err.contains("named fields"), "got: {err}");
    }

    #[test]
    fn enum_input_is_rejected() {
        let err = generate_err(r#"enum Test { A }"#);
        assert!(err.contains("only be derived for a `struct`"), "got: {err}");
    }

    #[test]
    fn unsigned_target_is_rejected() {
        let err = generate_err(r#"struct Test { #[column(from_i32)] a: u8 }"#);
        assert!(err.contains("cast_ints"), "got: {err}");
        let err = generate_err(r#"struct Test { #[column(from_i64)] b: Option<u64> }"#);
        assert!(err.contains("cast_ints"), "got: {err}");
    }

    #[test]
    fn flatten_uses_try_from_on_row() {
        let out = generate(r#"struct Test { #[column(flatten)] inner: Inner, plain: i64 }"#);
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("TryFrom::try_from(&mut*row)"),
            "got: {out}"
        );
    }
}
