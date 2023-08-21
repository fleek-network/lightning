use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parse;
use syn::Token;

pub struct IdentSetPair {
    pub left: IdentSet,
    pub comma: Token![,],
    pub right: IdentSet,
}

pub struct IdentSet {
    pub brace_token: syn::token::Brace,
    pub ident: syn::punctuated::Punctuated<syn::Ident, Token![,]>,
}

impl Parse for IdentSetPair {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            left: input.parse()?,
            comma: input.parse()?,
            right: input.parse()?,
        })
    }
}

impl Parse for IdentSet {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        Ok(Self {
            brace_token: syn::braced!(content in input),
            ident: content.parse_terminated(syn::Ident::parse, Token![,])?,
        })
    }
}

pub fn generate_partial_blank(pair: IdentSetPair) -> TokenStream {
    let mut string_to_ident = HashMap::<String, &syn::Ident>::new();
    for ident in pair.left.ident.iter() {
        string_to_ident.insert(ident.to_string(), ident);
    }
    for ident in pair.right.ident.iter() {
        string_to_ident.remove(&ident.to_string());
    }

    let result = string_to_ident.values();

    quote! {
        #(type #result = infusion::Blank<Self>;)*
    }
}

pub fn generate_partial_modifier(set: IdentSet) -> TokenStream {
    fn generate_code(current: syn::Ident, set: &IdentSet) -> TokenStream {
        let trait_name = syn::Ident::new(&format!("{current}Container"), current.span());
        let struct_name = syn::Ident::new(&format!("{current}Modifier"), current.span());
        let services = set.ident.iter().filter(|ident| *ident != &current);

        quote! {
            impl<N: #trait_name, O: CollectionBase> CollectionBase for #struct_name<N, O> {
                type #current<C: Collection> = N::#current<C>;
            #(
                type #services<C: Collection> = O::#services<C>;
            )*
            }
        }
    }

    let items = set
        .ident
        .iter()
        .map(|current| generate_code(current.clone(), &set));

    quote! {
        #(
        #items
        )*
    }
}
