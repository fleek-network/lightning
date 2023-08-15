use proc_macro2::{TokenStream, Span};
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, Error, Generics, ItemTrait, Result, TraitItem};

use crate::parse::Item;

mod parse;

macro_rules! err {
    ($o:ident, $span:expr, $msg:expr) => {{
        let err = Error::new($span, $msg).to_compile_error();
        $o.extend(quote! { #err });
    }};
}

#[proc_macro_attribute]
pub fn input(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    process(Mode::Input, attr, input)
}

#[proc_macro_attribute]
pub fn object(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    process(Mode::Object, attr, input)
}

#[proc_macro_attribute]
pub fn service(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    process(Mode::Service, attr, input)
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
enum Mode {
    Input,
    Object,
    Service,
}

fn process(
    mode: Mode,
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = parse_macro_input!(input as Item);
    let token = match item {
        Item::Trait(item) => process_trait(mode, item),
        _ => todo!(),
    };
    token.into()
}

fn process_trait(mode: Mode, mut trait_: ItemTrait) -> TokenStream {
    // The final output buffer. We can use it to collect errors.
    let mut output = quote!();

    // In this case we except the trait to have at least one generic.
    // The first one is the one that we are gonna use as the `Collection`
    let collection_ty_name = if mode == Mode::Input || mode == Mode::Service {
        let out = get_first_ident_from_generics(&trait_.generics);
        if out.is_none() {
            err!(output, trait_.generics.span(), "Missing Collection Generic");
        }

        out
    } else {
        None
    };

    // trait ... { %trait_body }
    let mut trait_body = Vec::<TraitItem>::new();
    // impl ... { %blank_body }
    let mut blank_body = quote!();

    let name = trait_.ident.clone();
    let mut has_init = false;
    let mut has_post = false;

    for item in std::mem::take(&mut trait_.items) {
        match item {
            TraitItem::Const(item) => {
                err!(blank_body, item.span(), "Const item not supported.");
                trait_body.push(TraitItem::Const(item));
            },

            TraitItem::Fn(f) => {
                let name = f.sig.ident.to_string();

                match name.as_str() {
                    "_init" => {
                        has_init = true;
                    },
                    "_post" => {
                        has_post = true;
                    },
                    _name if f.default.is_some() => {
                        // method has default implementation. no need to
                        // include it in blank.
                        trait_body.push(TraitItem::Fn(f));
                    },
                    _name => {
                        let impl_fn = transform_trait_fn_to_impl(&f);
                        trait_body.push(TraitItem::Fn(f));
                        blank_body.extend(quote! { #impl_fn });
                    },
                }
            },

            TraitItem::Type(mut item) => {
                let impl_ty = transform_trait_type_to_impl_type(&item);

                // Ignore the default for the trait type member since
                // it not supported in Rust.
                item.default = None;

                trait_body.push(TraitItem::Type(item));
                blank_body.extend(quote! { #impl_ty });
            },

            // handle anything else.
            // TraitItem::Macro(_) | TraitItem::Verbatim(_) | _
            item => {
                // non-exhaustive enum member. If we don't know what the item is or if we
                // don't have anything to do with it just push it to the trait.
                trait_body.push(item);
            },
        }
    }

    if !has_init {
        trait_body.push(TraitItem::Fn(
            syn::parse2(quote! {
                #[doc(hidden)]
                fn infu_dependencies(visitor: &mut infusion::graph::DependencyGraphVisitor) {
                    visitor.mark_input();
                }
            })
            .unwrap(),
        ));

        trait_body.push(TraitItem::Fn(
            syn::parse2(quote! {
                #[doc(hidden)]
                fn infu_initialize(
                    _container: &infusion::Container
                ) -> ::std::result::Result<
                    Self, ::std::boxed::Box<dyn std::error::Error>> where Self: Sized {
                    unreachable!("This trait is marked as an input.")
                }
            })
            .unwrap(),
        ));
    }

    if !has_post {
        trait_body.push(TraitItem::Fn(
            syn::parse2(quote! {
                #[doc(hidden)]
                fn infu_post_initialize(&mut self, container: &infusion::Container) {
                    // empty.
                }
            })
            .unwrap(),
        ));
    }

    // Set the trait items to what they are.
    trait_.items = trait_body;

    let blank = {
        let g = &trait_.generics;
        let names = extract_all_ident_from_generic(g);

        let params = if names.is_empty() {
            quote!()
        } else {
            quote! { <#(#names),*> }
        };

        let blank_generics = match names.as_slice() {
            [] => quote!(()),
            [a] => quote!(#a),
            names => quote!((#(#names),*))
        };

        quote! {
            impl #g #name #params for infusion::Blank<#blank_generics> {
                #blank_body
            }
        }
    };

    quote! {
        #output

        #trait_

        #blank
    }
}

// Find the first type from the generics and return the its identifier.
fn get_first_ident_from_generics(generics: &Generics) -> Option<syn::Ident> {
    for item in generics.params.iter() {
        if let syn::GenericParam::Type(ty) = item {
            return Some(ty.ident.clone());
        }
    }

    None
}

// Find the first type from the generics and return the its identifier.
fn extract_all_ident_from_generic(generics: &Generics) -> Vec<syn::Ident> {
    let mut result = Vec::new();

    for item in generics.params.iter() {
        if let syn::GenericParam::Type(ty) = item {
            result.push(ty.ident.clone());
        }
    }

    result
}

fn transform_fn_arg(base: &Option<syn::Ident>, arg: &syn::FnArg) -> syn::FnArg {
    match arg {
        syn::FnArg::Typed(_) => {
            todo!()
        },
        item => item.clone(),
    }
}

fn transform_trait_type_to_impl_type(item: &syn::TraitItemType) -> syn::ImplItemType {
    let blank_param = if let Some(ident) = get_first_ident_from_generics(&item.generics) {
        quote! { #ident }
    } else {
        quote! { () }
    };

    let (eq_token, ty) = item.default.clone().unwrap_or_else(|| {
        (
            syn::token::Eq::default(),
            syn::parse2(quote! { infusion::Blank<#blank_param> }).unwrap(),
        )
    });

    syn::ImplItemType {
        attrs: item.attrs.clone(),
        vis: syn::Visibility::Inherited,
        defaultness: None,
        type_token: item.type_token,
        ident: item.ident.clone(),
        generics: item.generics.clone(),
        eq_token,
        ty,
        semi_token: item.semi_token,
    }
}

fn transform_trait_fn_to_impl(item: &syn::TraitItemFn) -> syn::ImplItemFn {
    let block = syn::parse2(quote! {
        panic!("BLANK IMPLEMENTATION");
    })
    .unwrap();

    syn::ImplItemFn {
        attrs: item.attrs.clone(),
        vis: syn::Visibility::Inherited,
        defaultness: None,
        sig: item.sig.clone(),
        block,
    }
}

fn transform_init(item: syn::TraitItemFn) -> Result<TokenStream> {
    check_infu_trait_fn(&item)?;

    let mut dep_body = quote!();
    let mut init_body = quote!();

    todo!()
}

fn check_infu_trait_fn(item: &syn::TraitItemFn) -> Result<()> {
    if !item.sig.generics.params.is_empty() {
        return Err(Error::new(
            item.sig.generics.params.span(),
            "Generic parameters are not supported on an infu method.",
        ));
    }

    if let Some(c) = &item.sig.generics.where_clause {
        return Err(Error::new(
            c.span(),
            "Where clause is not supported on an infu method.",
        ));
    }

    Ok(())
}
