use syn::parse_macro_input;

use crate::parse::Item;

mod helpers;
mod on_trait;
mod parse;
mod sig;
mod utils;

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
        Item::Trait(trait_) => on_trait::process_trait(utils::Mode::WithCollection, trait_),
        Item::Impl(item) => syn::Error::new_spanned(item, "Infusion over impl not supported yet.")
            .to_compile_error(),
    };
    token.into()
}

// Given a set of identifiers as `__blank_helper!({A, B}, {A})` generate the `type % = Blank<Self>`
// for each item in the first set that is not in the second set.
#[proc_macro]
pub fn __blank_helper(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let pair = parse_macro_input!(input as helpers::IdentSetPair);
    helpers::generate_partial_blank(pair).into()
}

#[proc_macro]
pub fn __modifier_helper(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let set = parse_macro_input!(input as helpers::IdentSet);
    helpers::generate_partial_modifier(set).into()
}

#[proc_macro]
pub fn __gen_macros_helper(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let set = parse_macro_input!(input as helpers::IdentSet);
    helpers::generate_macros(set).into()
}
