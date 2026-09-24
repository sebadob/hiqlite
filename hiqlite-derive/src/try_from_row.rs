// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::from_row::{ColumnAttr, plan_fields};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::DeriveInput;

/// Generates `impl TryFrom<&mut hiqlite::Row<'_>>` for the validated row plan.
///
/// Same column mapping as `FromRow`, but every failure is returned as `hiqlite::Error` instead
/// of panicking: plain reads use `row.try_get`, and conversions that can fail (narrowing
/// integers, parsing) map their error to `Error::Sqlite`.
pub fn impl_try_from_row(input: DeriveInput) -> syn::Result<TokenStream> {
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
                    #id: row.try_get::<#ty>(#name)?,
                }
            }
            ColumnAttr::Flatten => quote! {
                #id: ::std::convert::TryFrom::try_from(&mut *row)?,
            },
            ColumnAttr::FromI32 => {
                let msg = format!("column '{col_name}' does not fit into i32");
                if field.is_opt {
                    quote! {
                        #id: row.try_get::<Option<i64>>(#name)?
                            .map(|i| <i32 as ::std::convert::TryFrom<i64>>::try_from(i))
                            .transpose()
                            .map_err(|_| ::hiqlite::Error::Sqlite(#msg.into()))?
                            .map(|i| i.into()),
                    }
                } else {
                    quote! {
                        #id: {
                            let i = row.try_get::<i64>(#name)?;
                            <i32 as ::std::convert::TryFrom<i64>>::try_from(i)
                                .map_err(|_| ::hiqlite::Error::Sqlite(#msg.into()))?
                                .into()
                        },
                    }
                }
            }
            ColumnAttr::FromI64 => {
                if field.is_opt {
                    quote! {
                        #id: row.try_get::<Option<i64>>(#name)?.map(|i| i.into()),
                    }
                } else {
                    quote! {
                        #id: row.try_get::<i64>(#name)?.into(),
                    }
                }
            }
            ColumnAttr::Parse => {
                let msg = format!("failed to parse column '{col_name}'");
                if field.is_opt {
                    quote! {
                        #id: row.try_get::<Option<String>>(#name)?
                            .map(|s| s.parse()
                                .map_err(|_| ::hiqlite::Error::Sqlite(#msg.into())))
                            .transpose()?,
                    }
                } else {
                    quote! {
                        #id: row.try_get::<String>(#name)?
                            .parse()
                            .map_err(|_| ::hiqlite::Error::Sqlite(#msg.into()))?,
                    }
                }
            }
            ColumnAttr::FromString => {
                if field.is_opt {
                    quote! {
                        #id: row.try_get::<Option<String>>(#name)?.map(|s| s.into()),
                    }
                } else {
                    quote! {
                        #id: row.try_get::<String>(#name)?.into(),
                    }
                }
            }
            ColumnAttr::Skip => quote! {
                #id: ::std::default::Default::default(),
            },
        };

        body.push(ts);
    }

    Ok(quote! {
        impl #impl_generics ::std::convert::TryFrom<&mut ::hiqlite::Row<'_>> for #name #ty_generics #where_clause {
            type Error = ::hiqlite::Error;

            #[inline]
            fn try_from(row: &mut ::hiqlite::Row<'_>) -> Result<Self, Self::Error> {
                Ok(Self { #(#body)* })
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    fn generate(input: &str) -> String {
        let input = parse_str::<DeriveInput>(input).unwrap();
        impl_try_from_row(input).unwrap().to_string()
    }

    #[test]
    fn basic_mapping_uses_try_get_by_column_name() {
        let out = generate(
            r#"struct Test { #[column(rename = "name_db")] name: String, skip_me: bool }"#,
        );
        // `quote!` renders with spaces around `::`, so shape checks run on the compact form.
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("TryFrom<&mut::hiqlite::Row"),
            "missing TryFrom impl: {out}"
        );
        assert!(
            compact.contains("try_get::<String>(\"name_db\")?"),
            "plain field not read via try_get with rename: {out}"
        );
        assert!(
            compact.contains("Ok(Self{"),
            "body must be wrapped in Ok(...) to satisfy the Result return type: {out}"
        );
    }

    #[test]
    fn from_i32_non_opt_returns_error_instead_of_panicking() {
        let out = generate(r#"struct Test { #[column(from_i32)] a: i32 }"#);
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("TryFrom<i64>>::try_from"),
            "missing try_from conversion: {out}"
        );
        assert!(
            compact.contains(".map_err("),
            "narrowing failure not mapped to an error: {out}"
        );
        assert!(
            !compact.contains(".expect("),
            "still panics on overflow: {out}"
        );
        assert!(
            out.contains("does not fit into i32"),
            "no error message: {out}"
        );
    }

    #[test]
    fn from_i32_opt_uses_transpose() {
        let out = generate(r#"struct Test { #[column(from_i32)] a: Option<i32> }"#);
        let compact = out.replace(' ', "");
        assert!(
            compact.contains(".transpose()"),
            "Option narrowing not transposed into the outer Result: {out}"
        );
        assert!(
            compact.contains("TryFrom<i64>>::try_from"),
            "missing try_from conversion: {out}"
        );
    }

    #[test]
    fn parse_non_opt_maps_parse_error() {
        let out = generate(r#"struct Test { #[column(parse)] a: u64 }"#);
        let compact = out.replace(' ', "");
        assert!(compact.contains(".parse()"), "missing parse call: {out}");
        assert!(
            compact.contains(".map_err("),
            "parse failure not mapped to an error: {out}"
        );
        assert!(
            !compact.contains(".expect("),
            "still panics on parse failure: {out}"
        );
        assert!(
            out.contains("failed to parse column 'a'"),
            "no parse error message: {out}"
        );
    }

    #[test]
    fn from_string_uses_into() {
        let out = generate(r#"struct Test { #[column(from_string)] a: MyType }"#);
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("try_get::<String>(\"a\")?.into()"),
            "from_string not converted via From<String>: {out}"
        );
    }

    #[test]
    fn skip_uses_default() {
        let out = generate(r#"struct Test { #[column(skip)] a: i64, b: i64 }"#);
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("Default::default()"),
            "skip field not defaulted: {out}"
        );
    }

    #[test]
    fn flatten_uses_try_from_on_row() {
        let out = generate(r#"struct Test { #[column(flatten)] inner: Inner, plain: i64 }"#);
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("TryFrom::try_from(&mut*row)?"),
            "flatten not converted via TryFrom<&mut Row>: {out}"
        );
    }

    #[test]
    fn impl_declares_hiqlite_error_as_error_type() {
        let out = generate(r#"struct Test { a: i64 }"#);
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("typeError=::hiqlite::Error;"),
            "associated Error type missing or wrong: {out}"
        );
    }

    #[test]
    fn generics_are_preserved() {
        // Where clauses on structs sit between the generics and the brace, like rustc requires.
        let out = generate(r#"struct Test<T> where T: Default { #[column(skip)] a: T }"#);
        let compact = out.replace(' ', "");
        assert!(
            compact.contains("forTest<T>"),
            "generics lost on impl: {out}"
        );
        assert!(
            compact.contains("whereT:Default"),
            "where clause lost: {out}"
        );
    }
}
