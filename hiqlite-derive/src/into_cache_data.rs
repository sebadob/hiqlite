use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput};

pub fn impl_cache_variants(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = input.ident;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(
            name.span(),
            "CacheVariants does not support generic enums",
        ));
    }

    let Data::Enum(data) = input.data else {
        return Err(syn::Error::new(
            name.span(),
            "CacheVariants can only be derived for a flat `enum` without variant values",
        ));
    };

    let mut name_matches = Vec::new();
    let mut names = Vec::new();

    for var in &data.variants {
        if !var.fields.is_empty() {
            return Err(syn::Error::new(
                var.ident.span(),
                "CacheVariants requires a flat enum: variant values are not supported",
            ));
        }
        let id = &var.ident;
        let name = id.to_string();
        name_matches.push(quote! {Self::#id => #name,});
        names.push(name);
    }

    // The cluster identifies a cache by its variant name. The set of names is a perfect hash,
    // solved here at compile time, so a node finds the handler of a name in O(1).
    let (key, disps, idxs) = if names.is_empty() {
        (0, Vec::new(), Vec::new())
    } else {
        let keys: Vec<&str> = names.iter().map(String::as_str).collect();
        let state = phf_generator::generate_hash(&keys);
        (state.key, state.disps, state.map)
    };
    let disps = disps.iter().map(|&(d1, d2)| quote! {(#d1, #d2)});

    Ok(quote! {
        impl ::hiqlite::CacheVariants for #name {
            #[inline]
            fn hiqlite_cache_name(&self) -> &'static str {
                match self {
                    #(#name_matches)*
                }
            }

            fn hiqlite_cache_names() -> &'static ::hiqlite::phf::OrderedSet<&'static str> {
                static NAMES: ::hiqlite::phf::OrderedSet<&'static str> = ::hiqlite::phf::OrderedSet {
                    map: ::hiqlite::phf::OrderedMap {
                        key: #key,
                        disps: &[#(#disps),*],
                        idxs: &[#(#idxs),*],
                        entries: &[#((#names, ())),*],
                    },
                };
                &NAMES
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    fn generate(src: &str) -> String {
        let input: DeriveInput = parse_str(src).unwrap();
        impl_cache_variants(input).unwrap().to_string()
    }

    fn generate_err(src: &str) -> String {
        let input: DeriveInput = parse_str(src).unwrap();
        impl_cache_variants(input).unwrap_err().to_string()
    }

    #[test]
    fn flat_enum_generates_names() {
        let out = generate("enum E { A, B, C }");
        let compact = out.replace(' ', "");
        assert!(compact.contains("Self::A=>\"A\""), "got: {out}");
        assert!(compact.contains("Self::C=>\"C\""), "got: {out}");
        assert!(
            compact.contains("entries:&[(\"A\",()),(\"B\",()),(\"C\",())]"),
            "got: {out}"
        );
    }

    #[test]
    fn empty_enum_generates_an_empty_set() {
        let out = generate("enum E {}");
        let compact = out.replace(' ', "");
        assert!(compact.contains("entries:&[]"), "got: {out}");
    }

    #[test]
    fn struct_input_is_rejected() {
        let err = generate_err("struct S {}");
        assert!(err.contains("flat `enum`"), "got: {err}");
    }

    #[test]
    fn generic_enum_is_rejected() {
        let err = generate_err("enum E<T> { A }");
        assert!(err.contains("generic enums"), "got: {err}");
    }

    #[test]
    fn fieldful_variant_is_rejected() {
        let err = generate_err("enum E { A(i64) }");
        assert!(
            err.contains("variant values are not supported"),
            "got: {err}"
        );
    }
}
