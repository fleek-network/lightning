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
///     // ...
///
///     if true {
///         panic!("nobody fought the foo!");
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

    let is_res = sig.output.to_token_stream().to_string().contains("Result");

    let force_write_impl = quote! {
        unsafe {
            sgxkit::fn0::output_data_clear();
            sgxkit::fn0::output_data_append(buf.as_ptr() as usize, buf.len());
        }
    };

    let new_ident = Ident::new(&format!("_{}", sig.ident), sig.ident.span());
    sig.ident = new_ident.clone();

    let main_impl = {
        if is_res {
            let span = sig.output.span();
            quote_spanned! {span=>
                let res = #new_ident();
                match res {
                    Ok(_) => {},
                    Err(e) => {
                        let string = format!("Error: {e}");
                        let buf = string.as_bytes();
                        #force_write_impl
                    },
                }
            }
        } else {
            quote! {
                #new_ident();
            }
        }
    };

    let panic_impl = quote! {
        // Clears output and writes "panicked at '$reason', src/main.rs:27:4"
        std::panic::set_hook(Box::new(|info| {
            let string = info.to_string();
            let buf = string.as_bytes();
            #force_write_impl
        }));
    };

    quote! {
        #(#attrs)*
        #vis #sig #block

        pub fn main() {
            #panic_impl
            #main_impl
        }
    }
    .into()
}
