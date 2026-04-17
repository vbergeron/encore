//! Codegen for `#[derive(ValueEncode)]`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, FieldsNamed, FieldsUnnamed, Ident, Variant};

use crate::attrs;
use crate::common;

const DERIVE_NAME: &str = "ValueEncode";

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
    let data = common::enum_data(input, DERIVE_NAME)?;

    let arms = data
        .variants
        .iter()
        .map(expand_variant)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(quote! {
        impl encore_vm::ffi::ValueEncode for #name {
            fn encode(
                &self,
                _vm: &mut encore_vm::vm::Vm,
            ) -> Result<encore_vm::value::Value, encore_vm::ffi::EncodeError> {
                match self {
                    #(#arms)*
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
        Fields::Unit => encode_nullary_enum(vname, &tag),
        Fields::Unnamed(fields) => encode_tuple_variant(vname, &tag, fields),
        Fields::Named(_) => unreachable!("rejected by reject_named_fields"),
    })
}

fn encode_nullary_enum(vname: &Ident, tag: &TokenStream) -> TokenStream {
    quote! {
        Self::#vname => Ok(encore_vm::value::Value::ctor(
            #tag,
            encore_vm::value::HeapAddress::NULL,
        )),
    }
}

fn encode_tuple_variant(vname: &Ident, tag: &TokenStream, fields: &FieldsUnnamed) -> TokenStream {
    let n = fields.unnamed.len();
    let bindings = common::field_bindings(n);
    let encodes = bindings.iter().enumerate().map(|(i, b)| quote! {
        _fields[#i] = encore_vm::ffi::ValueEncode::encode(#b, _vm)?;
    });
    quote! {
        Self::#vname(#(#bindings),*) => {
            let mut _fields = [encore_vm::value::Value::ZERO; #n];
            #(#encodes)*
            _vm.alloc_ctor(#tag, &_fields)
                .map_err(encore_vm::ffi::EncodeError::from)
        },
    }
}

// ── struct ────────────────────────────────────────────────────────────────────

fn expand_struct(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let data = common::struct_data(input, DERIVE_NAME)?;
    let tag = attrs::ctor_tag_on_struct(input)?;

    let body = match &data.fields {
        Fields::Unit => encode_unit_struct(&tag),
        Fields::Unnamed(fields) => encode_tuple_struct(&tag, fields),
        Fields::Named(fields) => encode_named_struct(&tag, fields),
    };

    Ok(quote! {
        impl encore_vm::ffi::ValueEncode for #name {
            fn encode(
                &self,
                _vm: &mut encore_vm::vm::Vm,
            ) -> Result<encore_vm::value::Value, encore_vm::ffi::EncodeError> {
                #body
            }
        }
    })
}

fn encode_unit_struct(tag: &TokenStream) -> TokenStream {
    quote! {
        Ok(encore_vm::value::Value::ctor(#tag, encore_vm::value::HeapAddress::NULL))
    }
}

fn encode_tuple_struct(tag: &TokenStream, fields: &FieldsUnnamed) -> TokenStream {
    let n = fields.unnamed.len();
    let indices = (0..n).map(syn::Index::from);
    let encodes = indices.enumerate().map(|(i, idx)| quote! {
        _fields[#i] = encore_vm::ffi::ValueEncode::encode(&self.#idx, _vm)?;
    });
    quote! {
        let mut _fields = [encore_vm::value::Value::ZERO; #n];
        #(#encodes)*
        _vm.alloc_ctor(#tag, &_fields).map_err(encore_vm::ffi::EncodeError::from)
    }
}

fn encode_named_struct(tag: &TokenStream, fields: &FieldsNamed) -> TokenStream {
    let n = fields.named.len();
    let names = fields.named.iter().map(|f| f.ident.as_ref().unwrap());
    let encodes = names.enumerate().map(|(i, name)| quote! {
        _fields[#i] = encore_vm::ffi::ValueEncode::encode(&self.#name, _vm)?;
    });
    quote! {
        let mut _fields = [encore_vm::value::Value::ZERO; #n];
        #(#encodes)*
        _vm.alloc_ctor(#tag, &_fields).map_err(encore_vm::ffi::EncodeError::from)
    }
}
