//! Procedural macros for the Encore VM FFI.
//!
//! This crate exposes two derives that bridge Rust enums to the VM's
//! constructor-tagged value representation. The expansion logic lives in the
//! [`encode`] and [`decode`] modules; this file is the proc-macro entry point.

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod attrs;
mod common;
mod decode;
mod encode;

/// Derive `ValueEncode` for an enum whose variants map to Encore VM constructors.
///
/// Each variant must be annotated with `#[ctor(TAG)]` where `TAG` is a `u8`
/// expression (typically a constant from the generated `ctors` module).
///
/// # Arity rules
/// - `Unit` variants      → nullary ctor, no heap allocation.
/// - `Unnamed(f0)`        → ctor with 1 field; `f0` must implement `ValueEncode`.
/// - `Unnamed(f0, f1)`    → ctor with 2 fields; all fields must implement `ValueEncode`.
/// - Up to 8 fields are supported; named fields are not.
///
/// # Example
/// ```ignore
/// #[derive(ValueEncode)]
/// enum Event {
///     #[ctor(ctors::INC_TAG)]   Inc,
///     #[ctor(ctors::DEC_TAG)]   Dec,
///     #[ctor(ctors::RESET_TAG)] Reset,
/// }
///
/// #[derive(ValueEncode)]
/// enum Effect {
///     #[ctor(ctors::BEEP_TAG)]  Beep,
///     #[ctor(ctors::PRINT_TAG)] Print(i32),   // 1 field
/// }
/// ```
#[proc_macro_derive(ValueEncode, attributes(ctor))]
pub fn derive_value_encode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    encode::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive `ValueDecode` for an enum whose variants map to Encore VM constructors.
///
/// Each variant must be annotated with `#[ctor(TAG)]` where `TAG` is a `u8`
/// expression matching the tag that the VM will produce.
///
/// # Arity rules
/// - `Unit` variants   → decoded from a nullary ctor (no fields read).
/// - `Unnamed(T)`      → decoded from a 1-field ctor; `T` must implement `ValueDecode`.
/// - `Unnamed(T, U)`   → decoded from a 2-field ctor; all fields must implement `ValueDecode`.
///
/// # Example
/// ```ignore
/// #[derive(ValueDecode)]
/// enum Effect {
///     #[ctor(ctors::BEEP_TAG)]  Beep,
///     #[ctor(ctors::PRINT_TAG)] Print(i32),
/// }
/// ```
#[proc_macro_derive(ValueDecode, attributes(ctor))]
pub fn derive_value_decode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    decode::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
