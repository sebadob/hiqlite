use quote::quote;
use syn::{Data, DeriveInput};

pub fn impl_cache_variants(input: DeriveInput) -> proc_macro::TokenStream {
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut index_matches = Vec::new();
    let mut variants_return = Vec::new();

    match input.data {
        Data::Enum(e) => {
            for (idx, var) in e.variants.iter().enumerate() {
                let i = idx as i32;
                let id = &var.ident;
                let name = id.to_string();

                index_matches.push(quote! {Self::#id => #i,});
                variants_return.push(quote! {(#i, #name)});
            }
        }
        Data::Struct(_) | Data::Union(_) => unimplemented!(),
    };

    quote! {
        impl ::hiqlite::CacheVariants for #impl_generics #name #ty_generics #where_clause {
            #[inline(always)]
            fn hiqlite_cache_index(&self) -> i32 {
                match self {
                    #(#index_matches)*
                }
            }

            fn hiqlite_cache_variants() -> &'static [(i32, &'static str)] {
                &[#(#variants_return),*]
            }
        }
    }
    .into()
}
