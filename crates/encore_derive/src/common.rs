//! Shared helpers for the `ValueEncode` / `ValueDecode` derives.
//!
//! Anything used by both codegen modules lives here so the derives stay
//! visually parallel and the validation rules are stated once.

use proc_macro2::Span;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Variant};

/// Ensure the input is an enum and return its body.
pub fn enum_data<'a>(input: &'a DeriveInput, derive_name: &str) -> syn::Result<&'a DataEnum> {
    match &input.data {
        Data::Enum(data) => Ok(data),
        _ => Err(syn::Error::new_spanned(
            &input.ident,
            format!("{derive_name} can only be derived for enums or structs"),
        )),
    }
}

/// Ensure the input is a struct and return its body.
pub fn struct_data<'a>(input: &'a DeriveInput, derive_name: &str) -> syn::Result<&'a DataStruct> {
    match &input.data {
        Data::Struct(data) => Ok(data),
        _ => Err(syn::Error::new_spanned(
            &input.ident,
            format!("{derive_name} can only be derived for enums or structs"),
        )),
    }
}

/// Returns true if the input is an enum.
pub fn is_enum(input: &DeriveInput) -> bool {
    matches!(input.data, Data::Enum(_))
}

/// Reject named-field variants with a derive-specific message.
///
/// Tuple and unit variants are the only supported shapes; named fields would
/// require choosing a field-order convention we deliberately don't expose.
pub fn reject_named_fields(variant: &Variant, derive_name: &str) -> syn::Result<()> {
    if matches!(variant.fields, syn::Fields::Named(_)) {
        return Err(syn::Error::new_spanned(
            &variant.ident,
            format!("{derive_name} does not support named fields; use tuple variants"),
        ));
    }
    Ok(())
}

/// Generate `_f0, _f1, ...` identifiers for binding tuple variant fields.
pub fn field_bindings(n: usize) -> Vec<syn::Ident> {
    (0..n)
        .map(|i| syn::Ident::new(&format!("_f{i}"), Span::call_site()))
        .collect()
}
