// This code is borrowed from:
// https://github.com/dtolnay/async-trait/blob/
// f07c8568702978c72d0c282f719a4649065ac517/src/parse.rs
//
// The purpose is to parse the attribute macro over both a
// `trait` and an `impl`.

use proc_macro2::Span;
use syn::parse::{Error, Parse, ParseStream, Result};
use syn::{Attribute, ItemImpl, ItemTrait, Token};

pub enum Item {
    Trait(ItemTrait),
    Impl(ItemImpl),
}

impl Parse for Item {
    fn parse(input: ParseStream) -> Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let mut lookahead = input.lookahead1();
        if lookahead.peek(Token![unsafe]) {
            let ahead = input.fork();
            ahead.parse::<Token![unsafe]>()?;
            lookahead = ahead.lookahead1();
        }
        if lookahead.peek(Token![pub]) || lookahead.peek(Token![trait]) {
            let mut item: ItemTrait = input.parse()?;
            item.attrs = attrs;
            Ok(Item::Trait(item))
        } else if lookahead.peek(Token![impl]) {
            let mut item: ItemImpl = input.parse()?;
            if item.trait_.is_none() {
                return Err(Error::new(Span::call_site(), "expected a trait impl"));
            }
            item.attrs = attrs;
            Ok(Item::Impl(item))
        } else {
            Err(lookahead.error())
        }
    }
}
