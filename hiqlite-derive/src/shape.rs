// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::from_row::plan_fields;
use proc_macro2::TokenStream;
use syn::DeriveInput;

/// Whether a release build runs the compile-time query checks: the `sql-check-release` feature of
/// this crate, which `hiqlite` turns on as `sql-check-release`.  It decides what a release build
/// compiles at all: the check itself, and with it the shapes a `sql_map!()` compares against.
pub(crate) const fn checks_run_in_release() -> bool {
    cfg!(feature = "sql-check-release")
}

/// The codegen of `#[derive(SqlShape)]`: the column names a struct reads, and nothing else.
///
/// A type mapped by a hand-written `From<&mut Row>` has no derive that could know its columns, and
/// the parent that flattens it needs exactly those names to publish its own shape.  This derive is
/// the way to hand them over: same fields, same `#[column(...)]` attributes and same validation as
/// `FromRow`, and it emits only the `SqlShape` impl - no mapping, no registration, no trait a
/// mapping has to run.
pub(crate) fn impl_shape(input: DeriveInput) -> syn::Result<TokenStream> {
    let plan = plan_fields(input, "SqlShape")?;
    Ok(crate::indexed::shape_impl(&plan))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn derive(code: &str) -> String {
        let input: DeriveInput = syn::parse_str(code).unwrap();
        impl_shape(input).unwrap().to_string()
    }

    fn parse_error(code: &str) -> String {
        let input: DeriveInput = syn::parse_str(code).unwrap();
        match impl_shape(input) {
            Ok(_) => panic!("the derive must not accept this input"),
            Err(e) => e.to_string(),
        }
    }

    /// Compact form of the generated tokens: `quote!` renders with spaces around `::`, `&` and `<`.
    fn compact(code: &str) -> String {
        code.replace(' ', "")
    }

    #[test]
    fn a_shape_only_derive_publishes_the_names_of_its_fields() {
        let out = derive(
            r#"
            struct Sub {
                #[column(rename = "sub_id")]
                id: i64,
                name: String,
                #[column(skip)]
                extra: Option<String>,
            }
            "#,
        );
        let c = compact(&out);
        assert!(
            c.contains("impl::hiqlite::SqlShapeforSub"),
            "missing the shape impl: {out}"
        );
        assert!(
            c.contains(r#"::hiqlite::sql_shape(&["name","sub_id"])"#),
            "missing the sorted names of the fields, with the rename applied: {out}"
        );
        assert!(
            !c.contains(r#"("extra",""#),
            "a skipped field reads no column and must not be in the shape: {out}"
        );
    }

    #[test]
    fn a_shape_only_derive_emits_no_mapping() {
        let out = derive("struct Sub { id: i64 }");
        let c = compact(&out);
        for item in [
            "FromRowIndexed",
            "TryFromRowIndexed",
            "fnfrom(row:",
            "register_hoisted",
            "FieldNames::new",
        ] {
            assert!(
                !c.contains(item),
                "a shape-only derive must emit {item}: {out}"
            );
        }
    }

    #[test]
    fn a_shape_only_derive_adds_the_shape_of_its_parts() {
        let out = derive(
            r"
            struct Outer {
                id: i64,
                #[column(flatten)]
                inner: Inner,
            }
            ",
        );
        let c = compact(&out);
        assert!(
            c.contains(".wrapping_add(<Inneras::hiqlite::SqlShape>::SHAPE)"),
            "the shape of a flattened part is missing from the total: {out}"
        );
        assert!(
            c.contains("whereInner:::hiqlite::SqlShape"),
            "a parent must ask its parts for a shape: {out}"
        );
    }

    #[test]
    fn a_shape_only_derive_rejects_an_enum() {
        let msg = parse_error("enum Outer { A, B }");
        assert!(
            msg.contains("SqlShape can only be derived for a `struct`"),
            "the message must name the derive in use: {msg}"
        );
    }

    #[test]
    fn a_shape_impl_is_emitted_only_where_a_check_can_run() {
        let c = compact(&derive("struct Sub { id: i64 }"));
        assert!(
            c.contains("#[cfg(debug_assertions)]impl::hiqlite::SqlShapeforSub"),
            "a release build that does not opt into the check must not carry a shape: {c}"
        );
    }

    #[test]
    fn a_shape_only_derive_keeps_the_generics_of_its_type() {
        let out = derive("struct Wrap<T> { value: T }");
        let c = compact(&out);
        assert!(
            c.contains("impl<T>::hiqlite::SqlShapeforWrap<T>"),
            "the shape impl must stay generic over the type parameters: {out}"
        );
    }
}
