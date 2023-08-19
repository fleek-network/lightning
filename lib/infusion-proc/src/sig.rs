use std::fmt::Display;

use syn::{spanned::Spanned, Error, Result};

/// The infusion function.
#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
pub enum InfuFnKind {
    Init,
    Post,
}

impl Display for InfuFnKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InfuFnKind::Init => write!(f, "init"),
            InfuFnKind::Post => write!(f, "post"),
        }
    }
}

/// Verify the signature of either init or post method. This method does not check the
/// name of the method.
///
/// Returns a list of dependencies that should be provided to the method.
pub fn verify_fn_signature(
    kind: InfuFnKind,
    sig: &syn::Signature,
) -> Result<(Vec<&syn::Ident>, Vec<&syn::Ident>)> {
    if !sig.generics.params.is_empty() {
        return Err(Error::new(
            sig.generics.params.span(),
            "Generic parameters are not supported on an infu method.",
        ));
    }

    if let Some(token) = &sig.generics.where_clause {
        return Err(Error::new(
            token.span(),
            "Where clause is not supported on an infu method.",
        ));
    }

    if let syn::ReturnType::Type(_, ty) = &sig.output {
        return Err(Error::new(
            ty.span(),
            "Infu init should not have explicit return type.",
        ));
    }

    if let Some(token) = &sig.asyncness {
        return Err(Error::new(
            token.span(),
            format!("infu::{kind} method not meant to be async."),
        ));
    }

    let mut receiver = None;
    let mut params = Vec::<&syn::Ident>::with_capacity(sig.inputs.len());
    let mut args = Vec::<&syn::Ident>::with_capacity(sig.inputs.len());

    for arg in &sig.inputs {
        match arg {
            syn::FnArg::Receiver(arg) => {
                receiver = Some(arg);
            },
            syn::FnArg::Typed(pat) => {
                let name = get_infu_param_name(pat)?;
                params.push(name);
                let arg = get_infu_collection_member_from_arg(pat)?;
                args.push(arg);
            },
        }
    }

    match (kind, receiver) {
        (InfuFnKind::Init, Some(arg)) => {
            return Err(Error::new(
                arg.span(),
                "infu::init method can not take self",
            ));
        },
        (InfuFnKind::Post, Some(arg)) if arg.reference.is_none() => {
            return Err(Error::new(
                arg.span(),
                "infu::post method must take self by reference",
            ));
        },
        (InfuFnKind::Post, Some(arg)) if arg.mutability.is_none() => {
            return Err(Error::new(
                arg.span(),
                "infu::post method must take self by mutable reference",
            ));
        },
        (InfuFnKind::Post, None) => {
            return Err(Error::new(
                sig.ident.span(),
                "infu::post method must take self.",
            ));
        },
        _ => {},
    }

    Ok((args, params))
}

fn get_infu_param_name(pat: &syn::PatType) -> Result<&syn::Ident> {
    match pat.pat.as_ref() {
        syn::Pat::Ident(path) => Ok(&path.ident),
        ty => Err(Error::new(ty.span(), "Invalid parameter name.")),
    }
}

fn get_infu_collection_member_from_arg(pat: &syn::PatType) -> Result<&syn::Ident> {
    match pat.ty.as_ref() {
        syn::Type::Path(syn::TypePath { qself: None, path }) => match path {
            ty @ syn::Path {
                leading_colon: None,
                ..
            } => Err(Error::new(ty.span(), "Missing leading double colon.")),
            syn::Path {
                leading_colon: Some(_),
                segments,
            } if segments.len() != 1 => Err(Error::new(
                segments.last().unwrap().span(),
                "Expected a single identifier.",
            )),
            syn::Path {
                leading_colon: Some(_),
                segments,
            } => {
                let seg = segments.first().unwrap();

                if !seg.arguments.is_empty() {
                    Err(Error::new(
                        segments.last().unwrap().span(),
                        "Did not expect arguments.",
                    ))
                } else {
                    Ok(&seg.ident)
                }
            },
        },
        ty => Err(Error::new(ty.span(), "Invalid type for infu method.")),
    }
}
