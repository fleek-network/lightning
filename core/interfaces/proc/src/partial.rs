use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::Parse;
use syn::Token;

pub struct IdentSet {
    #[allow(unused)]
    pub brace_token: syn::token::Brace,
    pub ident: syn::punctuated::Punctuated<syn::Ident, Token![,]>,
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

pub struct IdentSetPair {
    pub left: IdentSet,
    #[allow(unused)]
    pub comma: Token![,],
    pub right: IdentSet,
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

pub fn gen_missing_assignments(pair: IdentSetPair) -> TokenStream {
    let mut string_to_ident = HashMap::<String, &syn::Ident>::new();

    for ident in pair.left.ident.iter() {
        string_to_ident.insert(ident.to_string(), ident);
    }

    for ident in pair.right.ident.iter() {
        string_to_ident.remove(&ident.to_string());
    }

    let missing = string_to_ident.values();
    let hack = crate::hack_mod();

    quote! {
        #(type #missing = #hack::Blanket;)*
    }
}
