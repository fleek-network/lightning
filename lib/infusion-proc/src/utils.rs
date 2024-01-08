use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_quote, Error, Result};

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
pub enum Mode {
    WithCollection,
    BlankOnly,
}

/// Convert a trait `TraitItemFn` into a `ImplItemFn` using the provided block in place
/// of the implementation.
pub fn impl_trait_fn(item: &syn::TraitItemFn, block: syn::Block) -> syn::ImplItemFn {
    syn::ImplItemFn {
        attrs: item.attrs.clone(),
        vis: syn::Visibility::Inherited,
        defaultness: None,
        sig: item.sig.clone(),
        block,
    }
}

/// Convert a `TraitItemConst` into an `ImplItemConst`. Returns an error if the const
/// does not have a default.
pub fn impl_trait_const(item: &syn::TraitItemConst) -> Result<syn::ImplItemConst> {
    let Some((eq_token, expr)) = &item.default else {
        return Err(Error::new(
            item.ident.span(),
            "Const must have a default for the Blank.",
        ));
    };

    Ok(syn::ImplItemConst {
        attrs: item.attrs.clone(),
        vis: syn::Visibility::Inherited,
        defaultness: None,
        const_token: item.const_token,
        ident: item.ident.clone(),
        generics: item.generics.clone(),
        colon_token: item.colon_token,
        ty: item.ty.clone(),
        eq_token: *eq_token,
        expr: expr.clone(),
        semi_token: item.semi_token,
    })
}

/// Convert a `TraitItemType` into an `ImplItemType`. Sets the type to the appropriate `Blank`
/// instance depending on the generics present on the type item.
pub fn impl_trait_type(item: &syn::TraitItemType) -> syn::ImplItemType {
    let (eq_token, ty) = item.default.clone().unwrap_or_else(|| {
        (
            syn::token::Eq::default(),
            syn::parse2(generics_to_blank(&item.generics)).unwrap(),
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

/// Return a vector containing the name of the generic items.
pub fn collect_generics_names(generics: &syn::Generics) -> Vec<syn::Ident> {
    let mut result = Vec::new();

    for item in generics.params.iter() {
        if let syn::GenericParam::Type(ty) = item {
            result.push(ty.ident.clone());
        }
    }

    result
}

/// Create an instance of blank based on the provided generics.
///
/// <> -> infusion::Blank<()>
/// <T> -> infusion::Blank<T>
/// <(T),+> -> infusion::Blank<( (T),+ )>
pub fn generics_to_blank(generics: &syn::Generics) -> TokenStream {
    let names = collect_generics_names(generics);
    let blank_generics = match names.as_slice() {
        [] => quote!(()),
        [a] => quote!(#a),
        names => quote!((#(#names),*)),
    };
    quote!(infusion::Blank<#blank_generics>)
}

/// Create and return a function call to the `Tag::new` function.
pub fn tag(base: &syn::Ident, type_name: &syn::Ident) -> syn::Expr {
    parse_quote! {
        infusion::vtable::Tag::new::<#base :: #type_name>(
            stringify!(#type_name),
            <#base :: #type_name as #type_name <#base> > ::infu_dependencies
         )
    }
}
