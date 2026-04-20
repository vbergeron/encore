//! Core decoding trait and primitive `ValueDecode` impls.
//!
//! Heap-handle types (`VmList`, `VmBytes`) live in their own modules so this
//! file stays focused on the trait surface and primitives.

use crate::builtins::TAG_PAIR;
use crate::error::VmError;
use crate::value::Value;
use crate::vm::Vm;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },
    Vm,
}

impl From<VmError> for DecodeError {
    fn from(_value: VmError) -> Self {
        Self::Vm
    }
}

pub trait ValueDecode: Sized {
    fn decode(vm: &Vm, value: Value) -> Result<Self, DecodeError>;
}

impl ValueDecode for Value {
    fn decode(_vm: &Vm, value: Value) -> Result<Self, DecodeError> {
        Ok(value)
    }
}

impl ValueDecode for i32 {
    fn decode(_vm: &Vm, value: Value) -> Result<Self, DecodeError> {
        if !value.is_int() {
            return Err(DecodeError::TypeMismatch {
                expected: "int",
                got: value.type_name(),
            });
        }
        value.int_value().map_err(DecodeError::from)
    }
}

impl ValueDecode for bool {
    fn decode(_vm: &Vm, value: Value) -> Result<Self, DecodeError> {
        if !value.is_ctor() {
            return Err(DecodeError::TypeMismatch {
                expected: "bool (ctor tag 0 or 1)",
                got: value.type_name(),
            });
        }
        Ok(value.ctor_tag() != 0)
    }
}

impl<A: ValueDecode, B: ValueDecode> ValueDecode for (A, B) {
    fn decode(vm: &Vm, value: Value) -> Result<Self, DecodeError> {
        if !value.is_ctor() || value.ctor_tag() != TAG_PAIR {
            return Err(DecodeError::TypeMismatch {
                expected: "pair ctor",
                got: value.type_name(),
            });
        }
        let a = A::decode(vm, vm.ctor_field(value, 0))?;
        let b = B::decode(vm, vm.ctor_field(value, 1))?;
        Ok((a, b))
    }
}
