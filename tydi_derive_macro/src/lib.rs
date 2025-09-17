extern crate proc_macro;

use tydi_derive_core::tydi_derive_impl;
use proc_macro::TokenStream;

#[proc_macro_derive(Tydi, attributes(packed_size))]
pub fn tydi_derive(input: TokenStream) -> TokenStream {
    tydi_derive_impl(input.into()).into()
}
