use syn::parse_macro_input;

mod blank;
mod partial;

#[proc_macro_attribute]
pub fn blank(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let trait_ = parse_macro_input!(input as syn::ItemTrait);
    let token = blank::impl_blank(trait_);
    token.into()
}

/// Called like `macro!({A, B}, {A})` this macro generates the `type % = Blanket;` for every
/// ident from the first set that is missing from the second set.
#[proc_macro]
pub fn __gen_missing_assignments(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let pair = parse_macro_input!(input as partial::IdentSetPair);
    partial::gen_missing_assignments(pair).into()
}

pub(crate) fn hack_mod() -> syn::Path {
    let crate_name = std::env::var("CARGO_PKG_NAME").unwrap();
    if crate_name == "lightning-interfaces" {
        syn::parse_quote! {
            crate::_hacks
        }
    } else {
        syn::parse_quote! {
            lightning_interfaces::_hacks
        }
    }
}
