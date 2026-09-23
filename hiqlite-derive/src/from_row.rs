// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::spanned::Spanned;
use syn::{
    Attribute, Data, DeriveInput, GenericArgument, LitStr, Meta, MetaList, PathArguments, Type,
};

pub fn impl_from_row(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let Data::Struct(data) = input.data else {
        return Err(syn::Error::new(
            name.span(),
            "FromRow can only be derived for a `struct`",
        ));
    };

    let mut with_from_str = false;
    // (column name, field name) of every field that reads a column, for duplicate detection.
    let mut columns: Vec<(String, String)> = Vec::new();
    let mut body = Vec::new();

    for field in data.fields.iter() {
        let Some(id) = &field.ident else {
            return Err(syn::Error::new(
                field.ty.span(),
                "FromRow only supports structs with named fields; tuple fields are not supported",
            ));
        };

        let ch = ColumnHandler::from(field.attrs.as_slice())?;
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

        let name = if let Some(rename) = &ch.rename {
            rename.to_token_stream()
        } else {
            quote! {#col_name}
        };

        let ts = match ch.attr {
            ColumnAttr::Flatten => quote! {
                #id: ::std::convert::TryFrom::try_from(&mut *row).expect("failed to flatten column"),
            },
            ColumnAttr::FromI32 => {
                check_unsigned_target(&field.ty, is_opt, "i32")?;
                let msg = format!("column '{col_name}' does not fit into i32");
                let convert = quote! {
                    <i32 as ::std::convert::TryFrom<i64>>::try_from(i).expect(#msg).into()
                };
                if is_opt {
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
                check_unsigned_target(&field.ty, is_opt, "i64")?;
                if is_opt {
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
                with_from_str = true;
                let msg = format!("failed to parse column '{col_name}'");
                if is_opt {
                    quote! {
                        #id: row.get::<Option<String>>(#name)
                            .map(|s| s.parse().unwrap_or_else(|_| panic!(#msg))),
                    }
                } else {
                    quote! {
                        #id: row.get::<String>(#name).parse().unwrap_or_else(|_| panic!(#msg)),
                    }
                }
            }
            ColumnAttr::FromString => {
                if is_opt {
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
            ColumnAttr::None => quote! {
                #id: row.get(#name),
            },
        };

        body.push(ts);
    }

    let from_str_import = if with_from_str {
        quote! {use ::std::str::FromStr;}
    } else {
        quote! {}
    };

    Ok(quote! {
        impl #impl_generics From<&mut ::hiqlite::Row<'_>> for #name #ty_generics #where_clause {
            #[inline]
            fn from(row: &mut ::hiqlite::Row<'_>) -> Self {
                #from_str_import
                Self {
                    #(#body)*
                }
            }
        }
    })
}

fn check_unsigned_target(ty: &Type, is_opt: bool, kind: &str) -> syn::Result<()> {
    let Some(target) = conversion_target_name(ty, is_opt) else {
        return Ok(());
    };
    if matches!(target.as_str(), "u8" | "u16" | "u32" | "u64" | "usize") {
        let supported = if kind == "i32" {
            "i8, i16 and i32"
        } else {
            "i64 and i128"
        };
        return Err(syn::Error::new(
            ty.span(),
            format!(
                "`from_{kind}` converts via `From<{kind}>`, which std only implements for {supported}; \
                 for `{target}`, either drop the attribute and enable the `cast_ints` feature, or use `parse`"
            ),
        ));
    }
    Ok(())
}

/// Returns the name of the type a column value is converted into, with an `Option` wrapper
/// stripped (e.g. `u8` for both `u8` and `Option<u8>`).
fn conversion_target_name(ty: &Type, is_opt: bool) -> Option<String> {
    let ty = if is_opt { option_inner(ty)? } else { ty };
    let Type::Path(p) = ty else {
        return None;
    };
    p.path.segments.last().map(|seg| seg.ident.to_string())
}

/// Returns the type argument of an `Option<T>` field type.
fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(p) = ty else {
        return None;
    };
    let last = p.path.segments.last()?;
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

struct ColumnHandler {
    attr: ColumnAttr,
    rename: Option<LitStr>,
}

impl ColumnHandler {
    fn from(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut handler = Self {
            attr: ColumnAttr::None,
            rename: None,
        };
        for att in attrs {
            if !att.path().is_ident("column") {
                continue;
            }
            if handler.seen() {
                return Err(syn::Error::new(
                    att.span(),
                    "only one `#[column]` attribute per field is allowed; combine all options in a \
                     single attribute",
                ));
            }
            let Meta::List(MetaList { tokens, .. }) = &att.meta else {
                return Err(syn::Error::new(att.span(), "expected `#[column(...)]`"));
            };
            (handler.attr, handler.rename) = parse_column_list(tokens)?;
        }
        Ok(handler)
    }

    fn seen(&self) -> bool {
        !matches!(self.attr, ColumnAttr::None) || self.rename.is_some()
    }
}

/// Parses the token list of a single `#[column(...)]` attribute.
///
/// Items are comma-separated (a trailing comma is fine), in any order: one conversion keyword
/// (`skip`, `flatten`, `from_i32`, `from_i64`, `parse`, `from_string`) and/or
/// `rename = "some_column"`. The stream must be fully consumed.
fn parse_column_list(tokens: &TokenStream) -> syn::Result<(ColumnAttr, Option<LitStr>)> {
    let mut stream = tokens.clone().into_iter();
    let mut attr = ColumnAttr::None;
    let mut rename: Option<LitStr> = None;
    let mut items = 0u32;

    loop {
        // One item: a bare keyword or `rename = "..."`.
        let tree = match stream.next() {
            Some(tree) => tree,
            None => break,
        };
        let name = match &tree {
            TokenTree::Ident(ident) => ident.to_string(),
            other => {
                return Err(syn::Error::new(
                    tree.span(),
                    format!("expected a column keyword or `rename`, found `{other}`"),
                ));
            }
        };

        if name == "rename" {
            if rename.is_some() {
                return Err(syn::Error::new(
                    tree.span(),
                    "`rename` may only be used once per field",
                ));
            }
            if matches!(attr, ColumnAttr::Skip | ColumnAttr::Flatten) {
                return Err(syn::Error::new(
                    tree.span(),
                    "`rename` cannot be combined with `skip` or `flatten`",
                ));
            }
            // Expect `= "string literal"`.
            let Some(eq) = stream.next() else {
                return Err(syn::Error::new(tree.span(), "expected `=` after `rename`"));
            };
            if !matches!(&eq, TokenTree::Punct(p) if p.as_char() == '=') {
                return Err(syn::Error::new(eq.span(), "expected `=` after `rename`"));
            }
            let Some(lit) = stream.next() else {
                return Err(syn::Error::new(
                    tree.span(),
                    "expected a string literal after `rename =`",
                ));
            };
            let TokenTree::Literal(lit) = &lit else {
                return Err(syn::Error::new(
                    lit.span(),
                    "expected a string literal after `rename =`",
                ));
            };
            match syn::parse_str::<syn::Lit>(&lit.to_string()) {
                Ok(syn::Lit::Str(str_lit)) => rename = Some(str_lit),
                Ok(_) => {
                    return Err(syn::Error::new(
                        lit.span(),
                        "`rename` expects a string literal, e.g. `rename = \"my_column\"`",
                    ));
                }
                Err(e) => return Err(syn::Error::new(lit.span(), e.to_string())),
            }
        } else {
            let new_attr = match name.as_str() {
                "skip" => ColumnAttr::Skip,
                "flatten" => ColumnAttr::Flatten,
                "from_i32" => ColumnAttr::FromI32,
                "from_i64" => ColumnAttr::FromI64,
                "parse" => ColumnAttr::Parse,
                "from_string" => ColumnAttr::FromString,
                other => {
                    return Err(syn::Error::new(
                        tree.span(),
                        format!(
                            "unknown column attribute '{other}', expected one of: flatten, from_i32, \
                             from_i64, from_string, parse, rename = \"my_column\", skip"
                        ),
                    ));
                }
            };
            if !matches!(attr, ColumnAttr::None) {
                return Err(syn::Error::new(
                    tree.span(),
                    "only one conversion attribute per field is allowed; combine `rename` with it \
                     instead",
                ));
            }
            if matches!(new_attr, ColumnAttr::Skip | ColumnAttr::Flatten) && rename.is_some() {
                return Err(syn::Error::new(
                    tree.span(),
                    "`skip` and `flatten` cannot be combined with `rename`",
                ));
            }
            attr = new_attr;
        }
        items += 1;

        // Separator: end of list or `,`.
        match stream.next() {
            None => break,
            Some(TokenTree::Punct(p)) if p.as_char() == ',' => {}
            Some(t) => {
                return Err(syn::Error::new(
                    t.span(),
                    format!("expected `,` after column attribute `{name}`"),
                ));
            }
        }
    }

    if items == 0 {
        return Err(syn::Error::new(
            tokens.span(),
            "expected at least one column attribute",
        ));
    }

    Ok((attr, rename))
}

#[derive(PartialEq)]
enum ColumnAttr {
    None,
    Skip,
    Flatten,
    FromI32,
    FromI64,
    Parse,
    FromString,
}

fn is_field_ty_opt(ty: &Type) -> Option<bool> {
    let Type::Path(p) = ty else {
        return None;
    };
    // Last segment must be `Option`.
    if p.path.segments.last()?.ident != "Option" {
        return None;
    }
    match p.path.segments.len() {
        // Bare `Option<T>`.
        1 => Some(true),
        // `std::option::Option<T>` / `core::option::Option<T>`.
        3 if matches!(
            p.path.segments[0].ident.to_string().as_str(),
            "std" | "core"
        ) && p.path.segments[1].ident == "option" =>
        {
            Some(true)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    fn generate(src: &str) -> String {
        let input: DeriveInput = parse_str(src).unwrap();
        impl_from_row(input).unwrap().to_string()
    }

    fn generate_err(src: &str) -> String {
        let input: DeriveInput = parse_str(src).unwrap();
        impl_from_row(input).unwrap_err().to_string()
    }

    #[test]
    fn from_i32_uses_try_from_and_never_clamps() {
        let out = generate(
            r#"struct Test { #[column(from_i32)] a: i32, #[column(from_i32)] b: Option<i32> }"#,
        );
        // token streams render with spaces around `::`
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("TryFrom<i64>>::try_from"),
            "missing try_from: {out}"
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
    fn basic_mapping_uses_row_get_by_column_name() {
        let out = generate(
            r#"struct Test { #[column(rename = "name_db")] name: String, skip_me: bool }"#,
        );
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("From<&mut::hiqlite::Row"),
            "no From impl: {out}"
        );
        assert!(out.contains("name_db"), "rename not honored: {out}");
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
    fn duplicate_columns_are_rejected() {
        let err = generate_err(
            r#"struct Test { #[column(rename = "x")] a: i64, b: i64, #[column(rename = "x")] c: String }"#,
        );
        assert!(err.contains("duplicate column 'x'"), "got: {err}");
    }

    #[test]
    fn multiple_column_attrs_are_rejected() {
        let err =
            generate_err(r#"struct Test { #[column(from_i32)] #[column(rename = "a")] a: i16 }"#);
        assert!(err.contains("only one `#[column]`"), "got: {err}");
    }

    #[test]
    fn tuple_struct_is_rejected() {
        let err = generate_err("struct Test(i64);");
        assert!(err.contains("named fields"), "got: {err}");
    }

    #[test]
    fn enum_input_is_rejected() {
        let err = generate_err("enum Test { A }");
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
    fn skip_cannot_be_combined_with_rename() {
        let err = generate_err(r#"struct Test { #[column(skip, rename = "x")] a: i64 }"#);
        assert!(err.contains("cannot be combined"), "got: {err}");
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
