use proc_macro2::TokenStream;
use quote::quote;
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use syn::{spanned::Spanned, Error, GenericParam, Generics, ReturnType, TraitItem};

#[proc_macro_attribute]
pub fn blank(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    process(attr.into(), input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[derive(Deserialize)]
struct Config {
    object: Option<bool>,
}

fn process(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let attrs = from_tokenstream::<Config>(&attr)?;
    let mut trait_: syn::ItemTrait = syn::parse2::<syn::ItemTrait>(input.clone()).map_err(|e| {
        Error::new(
            input.span(),
            format!("#[blank] must be above a trait. \n{e}",),
        )
    })?;

    let is_object = attrs.object.unwrap_or(false);

    let name = trait_.ident.clone();
    let mut generic_types = quote!();
    let mut methods = quote!();

    for item in &mut trait_.items {
        match item {
            TraitItem::Fn(fun) => {
                if fun.default.is_some() {
                    continue;
                }

                let im = match fun.sig.output {
                    ReturnType::Default => {
                        quote!(())
                    },
                    _ => {
                        quote! {
                            todo!("Blank Implementation");
                        }
                    },
                };

                let sig = &fun.sig;

                methods = quote! {
                    #methods

                    #[allow(unused)]
                    #sig {
                        #im
                    }
                };
            },
            TraitItem::Type(ty) => {
                let name = &ty.ident;
                let num_generics = ty.generics.params.len();

                if num_generics > 0 && ty.default.is_none() {
                    return Err(Error::new(
                        ty.generics.span(),
                        "Default type required when generics are used.",
                    ));
                }

                let value = if let Some((eq, ty)) = &ty.default {
                    quote!(#eq #ty)
                } else {
                    quote!(= infusion::Blank<()>)
                };

                let generics = &ty.generics;

                // Trait does not accept default value.
                ty.default = None;

                generic_types = quote! {
                    #generic_types

                    type #name #generics #value;
                }
            },
            TraitItem::Const(c) => {
                return Err(Error::new_spanned(c, "#[blank] does not support const."));
            },
            _ => {},
        }
    }

    if !trait_.generics.params.is_empty() && !is_object {
        return Err(Error::new(
            trait_.generics.span(),
            "Infu service type not supposed to have generics.",
        ));
    }

    let output = if is_object {
        let generics = &trait_.generics;
        let where_clause = &generics.where_clause;
        let generic_names = extract_generic_names(generics)?;

        let (generics, generic_names, unit) = match generic_names.as_slice() {
            [] => (quote!(<T: 'static>), quote!(), quote!(T)),
            [name] => (quote!(#generics), quote!(<#name>), quote!(#name)),
            names => (
                quote!(#generics),
                quote!(<#(#names),*>),
                quote!((#(#names),*)),
            ),
        };

        quote! {
            #trait_

            impl #generics #name #generic_names for infusion::Blank<#unit> #where_clause {
                #generic_types

                #methods
            }
        }
    } else {
        quote! {
            #trait_

            impl<InfuCollection> #name for infusion::Blank<InfuCollection>
                where
            InfuCollection: Collection<#name = Self> {
                type Collection = InfuCollection;

                #generic_types

                fn infu_initialize(
                    _container: &infusion::Container
                ) -> Result<Self, Box<dyn std::error::Error>> where Self: Sized {
                    Ok(Self::default())
                }

                #methods
            }
        }
    };

    Ok(output)
}

fn extract_generic_names(generics: &Generics) -> syn::Result<Vec<syn::Ident>> {
    let mut result = Vec::with_capacity(generics.params.len());

    for param in &generics.params {
        match param {
            GenericParam::Type(tp) => result.push(tp.ident.clone()),
            e => return Err(syn::Error::new(e.span(), "Non-supported generic.")),
        }
    }

    Ok(result)
}
