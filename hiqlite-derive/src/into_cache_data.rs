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

    let mut index_matches = Vec::new();
    let mut variants_return = Vec::new();

    for (idx, var) in data.variants.iter().enumerate() {
        if !var.fields.is_empty() {
            return Err(syn::Error::new(
                var.ident.span(),
                "CacheVariants requires a flat enum: variant values are not supported",
            ));
        }
        let id = &var.ident;
        let name = id.to_string();
        index_matches.push(quote! {Self::#id => #idx,});
        variants_return.push(quote! {(#idx, #name)});
    }

    Ok(quote! {
        impl ::hiqlite::CacheVariants for #name {
            #[inline(always)]
            fn hiqlite_cache_index(&self) -> usize {
                match self {
                    #(#index_matches)*
                }
            }

            fn hiqlite_cache_variants() -> &'static [(usize, &'static str)] {
                &[#(#variants_return),*]
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
    fn flat_enum_generates_index_and_variants() {
        let out = generate("enum E { A, B, C }");
        let compact = out.replace(' ', "");
        assert!(compact.contains("hiqlite_cache_index"), "got: {out}");
        assert!(compact.contains("(0usize,\"A\")"), "got: {out}");
        assert!(compact.contains("(1usize,\"B\")"), "got: {out}");
        assert!(compact.contains("(2usize,\"C\")"), "got: {out}");
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
