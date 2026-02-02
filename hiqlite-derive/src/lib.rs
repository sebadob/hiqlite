// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use proc_macro2::{Literal, TokenStream, TokenTree};
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Meta, MetaList, parse_macro_input};

#[proc_macro_derive(FromRow, attributes(column))]
pub fn from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut with_from_str = false;

    let body = match input.data {
        Data::Struct(data) => data
            .fields
            .iter()
            .filter_map(|field| {
                let id = field.ident.as_ref()?;
                let ch = ColumnHandler::from(field.attrs.as_slice());

                let ts = match ch.attr {
                    ColumnAttr::Flatten => {
                        quote! {#id: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),}
                    }
                    ColumnAttr::FromI64 => {
                        // TODO I don't really like doing this 4 times, but (probably becuase
                        //  of lack of knowledge) some reason, when I use the `Literal.to_string()`
                        //  in advance, it always outputs an escaped string. The `Ident` behaves
                        //  differently.
                        if let Some(name) = ch.rename {
                            quote! {#id: row.get::<i64>(#name).into(),}
                        } else {
                            let name = id.to_string();
                            quote! {#id: row.get::<i64>(#name).into(),}
                        }
                    }
                    ColumnAttr::FromStr => {
                        with_from_str = true;
                        if let Some(name) = ch.rename {
                            quote! {#id: row.get::<String>(#name).parse().unwrap(),}
                        } else {
                            let name = id.to_string();
                            quote! {#id: row.get::<String>(#name).parse().unwrap(),}
                        }
                    }
                    ColumnAttr::FromString => {
                        if let Some(name) = ch.rename {
                            quote! {#id: row.get::<String>(#name).into(),}
                        } else {
                            let name = id.to_string();
                            quote! {#id: row.get::<String>(#name).into(),}
                        }
                    }
                    ColumnAttr::None => {
                        if let Some(name) = ch.rename {
                            quote! {#id: row.get(#name),}
                        } else {
                            let name = id.to_string();
                            quote! {#id: row.get(#name),}
                        }
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
    }.into()
}

struct ColumnHandler {
    rename: Option<Literal>,
    attr: ColumnAttr,
}

enum ColumnAttr {
    Flatten,
    FromI64,
    FromStr,
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
- from_i64
- from_str
- from_string
- rename = "my_column"
- skip
- rename may be combined with one of the from_* attributes
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
                                "from_i64" => attr = ColumnAttr::FromI64,
                                "from_str" => attr = ColumnAttr::FromStr,
                                "from_string" => attr = ColumnAttr::FromString,
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
                        "from_i64" => attr = ColumnAttr::FromI64,
                        "from_str" => attr = ColumnAttr::FromStr,
                        "from_string" => attr = ColumnAttr::FromString,
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
