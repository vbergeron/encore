//! Codegen for `#[derive(ValueDecode)]`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, FieldsNamed, FieldsUnnamed, Ident, Variant};

use crate::attrs;
use crate::common;

const DERIVE_NAME: &str = "ValueDecode";

pub fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    if common::is_enum(input) {
        expand_enum(input)
    } else {
        expand_struct(input)
    }
}

// ── enum ─────────────────────────────────────────────────────────────────────

fn expand_enum(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();
    let data = common::enum_data(input, DERIVE_NAME)?;

    let arms = data
        .variants
        .iter()
        .map(expand_variant)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(quote! {
        impl encore_vm::ffi::ValueDecode for #name {
            fn decode(
                _vm: &encore_vm::vm::Vm,
                _value: encore_vm::value::Value,
            ) -> Result<Self, encore_vm::ffi::DecodeError> {
                if !_value.is_ctor() {
                    return Err(encore_vm::ffi::DecodeError::TypeMismatch {
                        expected: #name_str,
                        got: _value.type_name(),
                    });
                }
                match _value.ctor_tag() {
                    #(#arms)*
                    _ => Err(encore_vm::ffi::DecodeError::TypeMismatch {
                        expected: #name_str,
                        got: "unknown-ctor",
                    }),
                }
            }
        }
    })
}

fn expand_variant(variant: &Variant) -> syn::Result<TokenStream> {
    common::reject_named_fields(variant, DERIVE_NAME)?;
    let tag = attrs::ctor_tag(variant)?;
    let vname = &variant.ident;
    Ok(match &variant.fields {
        Fields::Unit => decode_nullary_enum(vname, &tag),
        Fields::Unnamed(fields) => decode_tuple_variant(vname, &tag, fields),
        Fields::Named(_) => unreachable!("rejected by reject_named_fields"),
    })
}

fn decode_nullary_enum(vname: &Ident, tag: &TokenStream) -> TokenStream {
    quote! { t if t == #tag => Ok(Self::#vname), }
}

fn decode_tuple_variant(vname: &Ident, tag: &TokenStream, fields: &FieldsUnnamed) -> TokenStream {
    let decodes = fields.unnamed.iter().enumerate().map(|(i, f)| {
        let ty = &f.ty;
        quote! {
            <#ty as encore_vm::ffi::ValueDecode>::decode(_vm, _vm.ctor_field(_value, #i))?,
        }
    });
    quote! { t if t == #tag => Ok(Self::#vname(#(#decodes)*)), }
}

// ── struct ────────────────────────────────────────────────────────────────────

fn expand_struct(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();
    let data = common::struct_data(input, DERIVE_NAME)?;
    let tag = attrs::ctor_tag_on_struct(input)?;

    let body = match &data.fields {
        Fields::Unit => decode_unit_struct(),
        Fields::Unnamed(fields) => decode_tuple_struct(fields),
        Fields::Named(fields) => decode_named_struct(fields),
    };

    Ok(quote! {
        impl encore_vm::ffi::ValueDecode for #name {
            fn decode(
                _vm: &encore_vm::vm::Vm,
                _value: encore_vm::value::Value,
            ) -> Result<Self, encore_vm::ffi::DecodeError> {
                if !_value.is_ctor() || _value.ctor_tag() != #tag {
                    return Err(encore_vm::ffi::DecodeError::TypeMismatch {
                        expected: #name_str,
                        got: _value.type_name(),
                    });
                }
                Ok(#body)
            }
        }
    })
}

fn decode_unit_struct() -> TokenStream {
    quote! { Self }
}

fn decode_tuple_struct(fields: &FieldsUnnamed) -> TokenStream {
    let decodes = fields.unnamed.iter().enumerate().map(|(i, f)| {
        let ty = &f.ty;
        quote! {
            <#ty as encore_vm::ffi::ValueDecode>::decode(_vm, _vm.ctor_field(_value, #i))?,
        }
    });
    quote! { Self(#(#decodes)*) }
}

fn decode_named_struct(fields: &FieldsNamed) -> TokenStream {
    let decodes = fields.named.iter().enumerate().map(|(i, f)| {
        let fname = f.ident.as_ref().unwrap();
        let ty = &f.ty;
        quote! {
            #fname: <#ty as encore_vm::ffi::ValueDecode>::decode(_vm, _vm.ctor_field(_value, #i))?,
        }
    });
    quote! { Self { #(#decodes)* } }
}
