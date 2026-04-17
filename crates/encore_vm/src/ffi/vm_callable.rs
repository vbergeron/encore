use crate::ffi::{DecodeError, ValueDecode};
use crate::value::Value;
use crate::vm::Vm;

/// A decodable handle to a VM callable — either a flat `function` or a
/// heap-allocated `closure` (function + captured env). Both are invocable
/// the same way through [`Vm::call_closure`](crate::vm::Vm::call_closure).
#[derive(Clone, Copy, Debug)]
pub struct VmCallable(Value);

impl VmCallable {
    /// The underlying `Value` handle — useful for passing the callable into
    /// raw VM calls that take `&[Value]`.
    pub fn raw(self) -> Value {
        self.0
    }
}

impl ValueDecode for VmCallable {
    fn decode(_vm: &Vm, value: Value) -> Result<Self, DecodeError> {
        if !value.is_function() && !value.is_closure() {
            return Err(DecodeError::TypeMismatch {
                expected: "function or closure",
                got: value.type_name(),
            });
        }
        Ok(VmCallable(value))
    }
}
