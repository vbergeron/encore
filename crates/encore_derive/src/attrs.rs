//! Attribute parsing for the `encore_derive` proc-macros.
//!
//! Currently this only handles `#[ctor(TAG)]`, but new per-variant or
//! per-field attributes should land here so the codegen modules stay focused
//! on `TokenStream` construction.

use proc_macro2::TokenStream;
use syn::{Attribute, Variant};

/// Extract the tag expression from a variant's `#[ctor(TAG)]` attribute.
///
/// Returns a `syn::Error` spanned on the variant name when the attribute is
/// missing or unparseable, so callers can forward it as a `compile_error!`.
pub fn ctor_tag(variant: &Variant) -> syn::Result<TokenStream> {
    ctor_tag_from_attrs(&variant.attrs, &variant.ident)
}

/// Extract the tag expression from a struct's `#[ctor(TAG)]` attribute.
///
/// Returns a `syn::Error` spanned on the struct name when the attribute is
/// missing or unparseable.
pub fn ctor_tag_on_struct(input: &syn::DeriveInput) -> syn::Result<TokenStream> {
    ctor_tag_from_attrs(&input.attrs, &input.ident)
}

fn ctor_tag_from_attrs(attrs: &[Attribute], spanned: &impl quote::ToTokens) -> syn::Result<TokenStream> {
    match find_ctor_attr(attrs) {
        Some(tokens) => Ok(tokens),
        None => Err(syn::Error::new_spanned(
            spanned,
            "missing a `#[ctor(...)]` attribute",
        )),
    }
}

fn find_ctor_attr(attrs: &[Attribute]) -> Option<TokenStream> {
    for attr in attrs {
        if attr.path().is_ident("ctor") {
            return attr.parse_args::<TokenStream>().ok();
        }
    }
    None
}
