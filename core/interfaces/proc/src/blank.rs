use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_quote, Error, Result};

pub fn impl_blank(mut trait_: syn::ItemTrait) -> TokenStream {
    // The diagnostics and compile errors that we gather.
    let mut report = quote!();
    let mut impl_body = vec![];
    let mut has_async = false;
    let hack = crate::hack_mod();

    for item in trait_.items.iter_mut() {
        if let syn::TraitItem::Fn(item) = item {
            has_async |= item.sig.asyncness.is_some();
        }

        let result = match item {
            syn::TraitItem::Const(item) => apply(&hack, item, impl_const, syn::ImplItem::Const),
            syn::TraitItem::Type(item) => apply(&hack, item, impl_type, syn::ImplItem::Type),
            syn::TraitItem::Fn(item) => apply(&hack, item, impl_fn, syn::ImplItem::Fn),
            _ => continue,
        };

        match result {
            Ok(Some(item)) => impl_body.push(item),
            Ok(None) => {},
            Err(e) => report.extend(e.to_compile_error()),
        }
    }

    let blank = {
        let generics = &trait_.generics;
        let name = &trait_.ident;
        let where_clause = &trait_.generics.where_clause;
        let lt = &trait_.generics.lt_token;
        let gt = &trait_.generics.gt_token;

        let mut generic_params = quote!();
        for pair in generics.params.pairs() {
            let param = pair.value();
            let punc = pair.punct();

            match param {
                syn::GenericParam::Lifetime(param) => {
                    let item = &param.lifetime;
                    generic_params.extend(quote! { #item #punc })
                },
                syn::GenericParam::Type(param) => {
                    let item = &param.ident;
                    generic_params.extend(quote! { #item #punc })
                },
                syn::GenericParam::Const(param) => {
                    let item = &param.ident;
                    generic_params.extend(quote! { #item #punc })
                },
            }
        }

        quote! {
            #[allow(unused)]
            impl #generics #name #lt #generic_params #gt for #hack::Blanket #where_clause {
                #(#impl_body)*
            }
        }
    };

    let async_mod = has_async.then_some(&trait_.ident).map(|name| {
        quote! {
            #[trait_variant::make(#name: Send)]
        }
    });

    // Proxy identity to add trait_variant onto
    if has_async {
        trait_.ident = syn::Ident::new(&format!("_{}", trait_.ident), trait_.ident.span());
    }

    quote! {
        #report

        #async_mod
        #trait_

        #blank
    }
}

fn impl_const(
    _hack: &syn::Path,
    item: &mut syn::TraitItemConst,
) -> Result<Option<syn::ImplItemConst>> {
    let Some(maybe_expr) = extract_attribute_expr("blank", &mut item.attrs) else {
        return if item.default.is_some() {
            Ok(None)
        } else {
            Err(Error::new(
                item.span(),
                "Expected a blank assignment for constant without default.",
            ))
        };
    };

    let expr = maybe_expr?;

    Ok(Some(syn::ImplItemConst {
        attrs: vec![],
        vis: syn::Visibility::Inherited,
        defaultness: None,
        const_token: item.const_token,
        ident: item.ident.clone(),
        generics: item.generics.clone(),
        colon_token: item.colon_token,
        ty: item.ty.clone(),
        eq_token: <syn::Token![=]>::default(),
        expr,
        semi_token: <syn::Token![;]>::default(),
    }))
}

fn impl_fn(_hack: &syn::Path, item: &mut syn::TraitItemFn) -> Result<Option<syn::ImplItemFn>> {
    let expr = match (
        extract_attribute_expr("blank", &mut item.attrs),
        item.default.is_some(),
    ) {
        (None, true) => return Ok(None),
        (Some(Err(e)), _) => return Err(e),
        (None, false) => parse_quote! { panic!() },
        (Some(Ok(expr)), _) => expr,
    };

    let block = parse_quote! {
        {
            eprintln!("Blank Method Called!");
            #expr
        }
    };

    Ok(Some(syn::ImplItemFn {
        attrs: vec![],
        vis: syn::Visibility::Inherited,
        defaultness: None,
        sig: item.sig.clone(),
        block,
    }))
}

fn impl_type(hack: &syn::Path, item: &mut syn::TraitItemType) -> Result<Option<syn::ImplItemType>> {
    let ty = match (
        extract_attribute_type("blank", &mut item.attrs),
        item.default.is_some(),
    ) {
        (None, true) => return Ok(None),
        (Some(Err(e)), _) => return Err(e),
        (None, false) => parse_quote! { #hack::Blanket },
        (Some(Ok(ty)), _) => ty,
    };

    Ok(Some(syn::ImplItemType {
        attrs: item.attrs.clone(),
        vis: syn::Visibility::Inherited,
        defaultness: None,
        type_token: item.type_token,
        ident: item.ident.clone(),
        generics: item.generics.clone(),
        eq_token: syn::token::Eq::default(),
        ty,
        semi_token: item.semi_token,
    }))
}

/// Given the name of an attribute and a vec of attributes consume an attribute with that name
/// and return the express it has.
fn extract_attribute_expr(
    name: &str,
    attrs: &mut Vec<syn::Attribute>,
) -> Option<Result<syn::Expr>> {
    let index = attrs.iter().position(|attr| {
        let ident = match &attr.meta {
            syn::Meta::Path(_) => return false,
            syn::Meta::List(e) => e.path.get_ident(),
            syn::Meta::NameValue(e) => e.path.get_ident(),
        };

        ident.map(|ident| ident == name).unwrap_or(false)
    })?;

    match attrs.remove(index).meta {
        syn::Meta::Path(_) => unreachable!(),
        syn::Meta::NameValue(e) => Some(Ok(e.value)),
        syn::Meta::List(e) => Some(syn::parse2(e.tokens)),
    }
}

fn extract_attribute_type(
    name: &str,
    attrs: &mut Vec<syn::Attribute>,
) -> Option<Result<syn::Type>> {
    let index = attrs.iter().position(|attr| {
        let syn::Meta::List(name_value) = &attr.meta else {
            return false;
        };

        name_value
            .path
            .get_ident()
            .map(|ident| ident == name)
            .unwrap_or(false)
    })?;

    let syn::Meta::List(list) = attrs.remove(index).meta else {
        unreachable!()
    };

    Some(syn::parse2(list.tokens))
}

fn apply<C, T, I, O>(
    c: &C,
    v: &mut T,
    f: impl Fn(&C, &mut T) -> Result<Option<I>>,
    t: impl Fn(I) -> O,
) -> Result<Option<O>> {
    match f(c, v) {
        Ok(Some(v)) => Ok(Some(t(v))),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}
