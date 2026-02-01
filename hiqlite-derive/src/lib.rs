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
                let ts = match column_attr(&field.attrs) {
                    ColumnAttr::Flatten => {
                        quote! {#id: ::std::convert::TryFrom::try_from(&mut *row).unwrap(),}
                    }
                    ColumnAttr::FromI64 => {
                        let raw_str = id.to_string();
                        quote! {#id: row.get::<i64>(#raw_str).into(),}
                    }
                    ColumnAttr::FromStr => {
                        with_from_str = true;
                        let raw_str = id.to_string();
                        quote! {#id: row.get::<String>(#raw_str).parse().unwrap(),}
                    }
                    ColumnAttr::FromString => {
                        let raw_str = id.to_string();
                        quote! {#id: row.get::<String>(#raw_str).into(),}
                    }
                    ColumnAttr::None => {
                        let raw_str = id.to_string();
                        quote! {#id: row.get(#raw_str),}
                    }
                    ColumnAttr::Rename(rename) => quote! {#id: row.get(#rename),},
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

// struct ColumnHandler {
//     rename: Option<Literal>,
//     attr: ColumnAttr,
// }

enum ColumnAttr {
    Flatten,
    FromI64,
    FromStr,
    FromString,
    None,
    Rename(Literal),
    Skip,
}

fn column_attr(attrs: &[Attribute]) -> ColumnAttr {
    attrs
        .iter()
        .find_map(|attr| {
            if let Meta::List(MetaList { path, tokens, .. }) = &attr.meta
                && path.segments.first()?.ident == "column"
            {
                let mut tokens = tokens.clone().into_iter();
                match tokens.next()?.to_string().as_str() {
                    "flatten" => return Some(ColumnAttr::Flatten),
                    "from_i64" => return Some(ColumnAttr::FromI64),
                    "from_str" => return Some(ColumnAttr::FromStr),
                    "from_string" => return Some(ColumnAttr::FromString),
                    "rename" => {
                        if matches!(tokens.next()?, TokenTree::Punct(p) if p.as_char() == '=')
                            && let TokenTree::Literal(lit) = tokens.next()?
                        {
                            return Some(ColumnAttr::Rename(lit));
                        } else {
                            panic!(
                                r#"
Invalid syntax for '#[column(rename)]', expected something like:

#[column(rename = "my_column")]
"#
                            );
                        }
                    }
                    "skip" => return Some(ColumnAttr::Skip),
                    _ => {
                        panic!(
                            r#"
Invalid syntax for '#[column]' attribute, expected one of:

- flatten
- from_i64
- from_str
- from_string
- rename = "my_column"
- skip
"#
                        );
                    }
                }
            }
            None
        })
        .unwrap_or(ColumnAttr::None)
}
