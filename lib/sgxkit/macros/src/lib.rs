use proc_macro::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;
use syn::{parse_macro_input, Ident, ItemFn};

/// Declare a main function, with panic and error result handling setup.
///
/// # Example
///
/// ```
/// #[sgxkit::main]
/// fn main() -> Result<(), &str> {
///     if false {
///         return Err("...");
///     }
///
///     if false {
///         panic!("...");
///     }
///
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let ItemFn {
        attrs,
        vis,
        mut sig,
        block,
    } = parse_macro_input!(item as ItemFn);

    if sig.asyncness.is_some() {
        return quote!(compile_error!("cannot be async")).into();
    }

    let new_ident = Ident::new(&format!("_{}", sig.ident), sig.ident.span());
    sig.ident = new_ident.clone();

    let main_impl = {
        if sig.output.to_token_stream().to_string().contains("Result") {
            let span = sig.output.span();
            quote_spanned! {span=>
                let res = #new_ident();
                match res {
                    Ok(v) => v,
                    Err(e) => panic!("Error: {e}"),
                }
            }
        } else {
            quote! { #new_ident(); }
        }
    };

    quote! {
        #(#attrs)*
        #vis #sig #block

        pub fn main() {
            sgxkit::panic::set_default_hook();
            #main_impl
        }
    }
    .into()
}
