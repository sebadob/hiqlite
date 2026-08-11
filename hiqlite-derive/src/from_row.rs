use proc_macro2::{Literal, TokenStream, TokenTree};
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Meta, MetaList, Type};

pub fn impl_from_row(input: DeriveInput) -> proc_macro2::TokenStream {
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut with_from_str = false;

    let body = match input.data {
        Data::Struct(data) => data
            .fields
            .iter()
            .filter_map(|field| {
                let id = field.ident.as_ref()?;
                let is_opt = is_field_ty_opt(&field.ty).unwrap_or(false);
                let ch = ColumnHandler::from(field.attrs.as_slice());
                let name = if let Some(name) = &ch.rename {
                    quote! {#name}
                } else {
                    let name = id.to_string();
                    quote! {#name}
                };

                let ts = match ch.attr {
                    ColumnAttr::Flatten => {
                        quote! {#id: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),}
                    }
                    ColumnAttr::FromI32 => {
                        // out-of-range values must fail loudly instead of being silently
                        // clamped to a wrong value
                        let convert = quote! {
                            <i32 as ::std::convert::TryFrom<i64>>::try_from(i)
                                .expect("column value does not fit into i32")
                                .into()
                        };
                        if is_opt {
                            quote! {
                                #id: row.get::<Option<i64>>(#name).map(|i| #convert),
                            }
                        } else {
                            quote! {#id: {
                                let i = row.get::<i64>(#name);
                                #convert
                            },}
                        }
                    }
                    ColumnAttr::FromI64 => {
                        if is_opt {
                            quote! {#id: row.get::<Option<i64>>(#name).map(|i| i.into()),}
                        } else {
                            quote! {#id: row.get::<i64>(#name).into(),}
                        }
                    }
                    ColumnAttr::Parse => {
                        with_from_str = true;
                        if is_opt {
                            quote! {
                                #id: row.get::<Option<String>>(#name).map(|s| s.parse().unwrap()),
                            }
                        } else {
                            quote! {#id: row.get::<String>(#name).parse().unwrap(),}
                        }
                    }
                    ColumnAttr::FromString => {
                        if is_opt {
                            quote! {#id: row.get::<Option<String>>(#name).map(|s| s.into()),}
                        } else {
                            quote! {#id: row.get::<String>(#name).into(),}
                        }
                    }
                    ColumnAttr::None => {
                        quote! {#id: row.get(#name),}
                    }
                    ColumnAttr::Skip => quote! {#id: ::std::default::Default::default(),},
                };
                Some(ts)
            })
            .collect::<Vec<TokenStream>>(),
        Data::Enum(_) => unimplemented!(),
        Data::Union(_) => unimplemented!(),
    };

    let from_str = if with_from_str {
        quote! {use ::std::str::FromStr;}
    } else {
        quote! {}
    };

    quote! {
        impl #impl_generics ::std::convert::From<&mut ::hiqlite::Row<'_>> for #name #ty_generics #where_clause {
            #[inline]
            fn from(row: &mut ::hiqlite::Row) -> Self {
                #from_str
                Self {
                    #(#body)*
                }
            }
        }
    }
}

#[inline]
fn is_field_ty_opt(ty: &Type) -> Option<bool> {
    let Type::Path(ty) = ty else {
        return Some(false);
    };
    let mut iter = ty.path.segments.iter();

    let mut s = iter.next()?.ident.to_string();
    if s == "std" {
        s = iter.next()?.ident.to_string();
    }
    if s == "option" {
        s = iter.next()?.ident.to_string();
    }
    Some(s.as_str() == "Option")
}

struct ColumnHandler {
    rename: Option<Literal>,
    attr: ColumnAttr,
}

enum ColumnAttr {
    Flatten,
    FromI32,
    FromI64,
    Parse,
    FromString,
    None,
    Skip,
}

impl From<&[Attribute]> for ColumnHandler {
    fn from(attrs: &[Attribute]) -> Self {
        let mut rename: Option<Literal> = None;
        let mut attr = ColumnAttr::None;

        let do_panic = |idx: String| {
            panic!(
                r#"
Invalid syntax for '#[column]' - '{idx}' attribute, expected one of:

- flatten
- from_i32
- from_i64
- from_string
- parse
- rename = "my_column"
- skip
- rename may be combined with one of the from_* or parse attributes
"#
            )
        };

        for att in attrs {
            let Meta::List(MetaList { path, tokens, .. }) = &att.meta else {
                continue;
            };
            if let Some(seg) = path.segments.first()
                && seg.ident != "column"
            {
                continue;
            }

            let mut stream = tokens.clone().into_iter();
            let Some(tree) = stream.next() else {
                do_panic("missing first argument".to_string());
                break;
            };
            let value = tree.to_string();
            match value.as_str() {
                "flatten" => attr = ColumnAttr::Flatten,
                "skip" => attr = ColumnAttr::Skip,
                "rename" => {
                    if matches!(stream.next(), Some(TokenTree::Punct(p)) if p.as_char() == '=')
                        && let Some(TokenTree::Literal(lit)) = stream.next()
                    {
                        rename = Some(lit);

                        // check possibly following from_* attr
                        if let Some(tree) = stream.next() {
                            let TokenTree::Punct(p) = tree else {
                                do_panic("Invalid punctuation after rename".to_string());
                                break;
                            };
                            if p.as_char() != ',' {
                                do_panic(
                                    "Invalid punctuation after rename, expected ','".to_string(),
                                );
                            }
                            let Some(tree) = stream.next() else {
                                do_panic("Missing value after rename".to_string());
                                break;
                            };
                            let value = tree.to_string();
                            match value.as_str() {
                                "from_i32" => attr = ColumnAttr::FromI32,
                                "from_i64" => attr = ColumnAttr::FromI64,
                                "from_string" => attr = ColumnAttr::FromString,
                                "parse" => attr = ColumnAttr::Parse,
                                _ => do_panic(format!(
                                    "Invalid syntax for 'from_*' after 'rename': {value}"
                                )),
                            }
                        }
                    } else {
                        do_panic("cannot parse 'rename'".to_string());
                    }
                }
                other => {
                    match other {
                        "from_i32" => attr = ColumnAttr::FromI32,
                        "from_i64" => attr = ColumnAttr::FromI64,
                        "from_string" => attr = ColumnAttr::FromString,
                        "parse" => attr = ColumnAttr::Parse,
                        _ => do_panic(format!("Invalid syntax for 'from_*': {other}")),
                    }
                    if let Some(tree) = stream.next() {
                        let TokenTree::Punct(p) = tree else {
                            do_panic("Invalid punctuation after rename".to_string());
                            break;
                        };
                        if p.as_char() != ',' {
                            do_panic("Invalid punctuation after rename, expected ','".to_string());
                        }
                        let Some(tree) = stream.next() else {
                            do_panic("Missing value after rename".to_string());
                            break;
                        };
                        let value = tree.to_string();
                        if value != "rename" {
                            do_panic(format!(
                                "from_* attributes can only be combined with a rename, found: {value}"
                            ));
                            break;
                        }

                        if matches!(stream.next(), Some(TokenTree::Punct(p)) if p.as_char() == '=')
                            && let Some(TokenTree::Literal(lit)) = stream.next()
                        {
                            rename = Some(lit);
                        } else {
                            do_panic("cannot parse 'rename' after 'from_*'".to_string());
                        }
                    }
                }
            }
        }

        Self { rename, attr }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn generate(src: &str) -> String {
        let input: DeriveInput = syn::parse_str(src).unwrap();
        impl_from_row(input).into_token_stream().to_string()
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
}
