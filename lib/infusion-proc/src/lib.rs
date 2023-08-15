use proc_macro2::TokenStream;
use quote::quote;
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use syn::{spanned::Spanned, Error, GenericParam, Generics, ReturnType, TraitItem};

#[proc_macro_attribute]
pub fn input(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    process(Mode::Input, attr.into(), input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn object(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    process(Mode::Object, attr.into(), input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn service(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    process(Mode::Service, attr.into(), input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
enum Mode {
    Input,
    Object,
    Service
}

fn process(mode: Mode, attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    todo!()
}

