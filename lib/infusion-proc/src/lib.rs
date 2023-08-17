#![feature(box_patterns)]

use syn::parse_macro_input;

use crate::parse::Item;

mod on_trait;
mod parse;
mod sig;
mod utils;
mod sub;

#[proc_macro_attribute]
pub fn blank(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let trait_ = parse_macro_input!(input as syn::ItemTrait);
    let token = on_trait::process_trait(utils::Mode::BlankOnly, trait_);
    token.into()
}

#[proc_macro_attribute]
pub fn service(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = parse_macro_input!(input as Item);
    let token = match item {
        Item::Trait(item) => on_trait::process_trait(utils::Mode::WithCollection, item),
        _ => todo!(),
    };
    token.into()
}

#[proc_macro]
pub fn x(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let pair = parse_macro_input!(input as sub::IdentSetPair);
    sub::generate_partial_blank(pair).into()
}
