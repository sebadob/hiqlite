// Copyright 2026 Sebastian Dobe <sebastiandobe@mailbox.org>

use crate::from_row::impl_from_row;
use syn::{DeriveInput, parse_macro_input};

mod from_row;

#[proc_macro_derive(FromRow, attributes(column))]
pub fn from_row(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    // TODO could be reworked into returning a result, which would make the impl a bit cleaner
    impl_from_row(parse_macro_input!(input as DeriveInput))
}
