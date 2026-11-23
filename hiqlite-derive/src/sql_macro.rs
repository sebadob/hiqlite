// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::check::Check;
use crate::check::QueryCheck;
use crate::check::Schema;
use crate::check::Watch;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::ToTokens;
use quote::quote;
use syn::LitStr;
use syn::Token;
use syn::Type;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse::Result as SynResult;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;

/// The shape a validated query string must have.
#[derive(Clone, Copy)]
pub(crate) enum Expansion {
    /// Exactly one statement, the shape of `execute`, `query` and all their variants.
    Single,
    /// Any number of statements, the shape of `batch` and `txn`.
    Batch,
    /// One statement, and its result columns checked against the fields of a mapped type.
    Mapped,
}

/// The invocation of `sql_map!`: the type a query maps to, and the query.
struct Mapped {
    /// The type whose fields the result columns are checked against.
    ty: Type,
    /// The query string.
    sql: LitStr,
}

impl Parse for Mapped {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let ty = input.parse()?;
        input.parse::<Token![,]>()?;
        let sql = input.parse()?;
        Ok(Self { ty, sql })
    }
}

impl Expansion {
    /// Validate the query and hand it back untouched.
    ///
    /// Success re-emits the string literal, byte for byte, together with one `include_str!` per
    /// migration file: that registers the migrations as dependencies of the crate, which is what
    /// makes cargo recompile it when a migration changes. `sql_map!` adds the `const` comparison of
    /// the query's result columns against the field names of the type it maps.
    pub(crate) fn expand(self, input: TokenStream) -> TokenStream {
        if let Self::Mapped = self {
            return Self::expand_mapped(input);
        }

        // A trailing comma is allowed, the way rustc's own macros take one.
        let parts =
            parse_macro_input!(input with Punctuated::<LitStr, Token![,]>::parse_terminated);
        let Some(lit) = parts.iter().next().filter(|_| parts.len() == 1) else {
            return Self::malformed(self, Span::call_site());
        };
        let check = Check {
            sql: &lit.value(),
            single: matches!(self, Self::Single),
            columns: false,
        };
        match QueryCheck::validate(&check) {
            Ok(done) if done.checked => Self::emit(lit, &quote!()),
            // Nothing was checked, so the migration files are not worth watching either.
            Ok(_) => lit.to_token_stream().into(),
            Err(msg) => syn::Error::new(lit.span(), msg).to_compile_error().into(),
        }
    }

    /// Validate a `sql_map!` call: the query, and the columns it returns against the type.
    fn expand_mapped(input: TokenStream) -> TokenStream {
        let mapped = parse_macro_input!(input as Mapped);
        let sql = mapped.sql.value();
        let check = Check {
            sql: &sql,
            single: true,
            columns: true,
        };
        match QueryCheck::validate(&check) {
            Ok(done) if done.checked => {
                Self::emit(&mapped.sql, &Self::shape(&mapped, &done.columns))
            }
            Ok(_) => mapped.sql.to_token_stream().into(),
            Err(msg) => syn::Error::new(mapped.sql.span(), msg)
                .to_compile_error()
                .into(),
        }
    }

    /// The `const` comparison of the columns of the query against the shape of the mapped type.
    ///
    /// The compiler evaluates it, which is the only way the field names of a type can take part: a
    /// macro sees a type path, never the fields behind it. A release build that opted into
    /// `sql-check-release` gets the comparison unconditionally; a debug build always gets it, and a
    /// plain release build gets nothing at all.
    fn shape(mapped: &Mapped, columns: &[String]) -> proc_macro2::TokenStream {
        let ty = &mapped.ty;
        let names = columns
            .iter()
            .map(|name| LitStr::new(name, Span::call_site()));
        let msg = format!(
            "this query returns the columns ({}) - the mapped type must read exactly these, in any \
             order and case; `sql_map!` checks them at compile time",
            columns.join(", "),
        );
        let gate = if QueryCheck::is_forced() {
            quote!()
        } else {
            quote!(#[cfg(debug_assertions)])
        };
        quote! {
            #gate
            const _: () = assert!(
                ::hiqlite::sql_shape(&[ #(#names),* ]) == <#ty as ::hiqlite::SqlShape>::SHAPE,
                #msg
            );
        }
    }

    /// The output of a validated literal: the string untouched, the migration watch tokens, and
    /// whatever extra the macro shape adds.
    fn emit(sql: &LitStr, extra: &proc_macro2::TokenStream) -> TokenStream {
        let watch = Watch::tokens(Schema::current());
        quote! {
            {
                #(#watch)*
                #extra
                #sql
            }
        }
        .into()
    }

    /// The message of an invocation this macro cannot take.
    fn malformed(self, span: Span) -> TokenStream {
        let msg = format!("`{}` takes exactly one string literal", Self::name(self));
        syn::Error::new(span, msg).to_compile_error().into()
    }

    /// The name users write the macro as, for the message of a malformed invocation.
    fn name(self) -> &'static str {
        match self {
            Self::Single => "sql!",
            Self::Batch => "sql_batch!",
            Self::Mapped => "sql_map!",
        }
    }
}
