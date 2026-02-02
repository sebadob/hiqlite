use quote::quote;
use syn::{Data, DeriveInput};

pub fn impl_cache_variants(input: DeriveInput) -> proc_macro::TokenStream {
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut ts = Vec::new();
    match input.data {
        Data::Enum(e) => {
            let mut idx = 0;
            for var in e.variants {
                let name = var.ident.to_string();
                ts.push(quote! {(#idx, #name)});
                idx += 1;
            }
        }
        Data::Struct(_) | Data::Union(_) => unimplemented!(),
    };

    quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            pub fn hiqlite_cache_variants() -> &'static [(i32, &'static str)] {
                &[#(#ts),*]
            }
        }
    }
    .into()
}
