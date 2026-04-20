use crate::error::VmError;
use crate::value::Value;
use crate::vm::Vm;

const MIN_VM_INT: i32 = -8_388_608;
const MAX_VM_INT: i32 = 8_388_607;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncodeError {
    IntOutOfRange(i32),
    Vm,
}

impl From<VmError> for EncodeError {
    fn from(_value: VmError) -> Self {
        Self::Vm
    }
}

pub trait ValueEncode {
    fn encode(&self, vm: &mut Vm) -> Result<Value, EncodeError>;
}

impl ValueEncode for Value {
    fn encode(&self, _vm: &mut Vm) -> Result<Value, EncodeError> {
        Ok(*self)
    }
}

impl ValueEncode for i32 {
    fn encode(&self, _vm: &mut Vm) -> Result<Value, EncodeError> {
        if !(MIN_VM_INT..=MAX_VM_INT).contains(self) {
            return Err(EncodeError::IntOutOfRange(*self));
        }
        Ok(Value::int(*self))
    }
}

impl ValueEncode for bool {
    fn encode(&self, _vm: &mut Vm) -> Result<Value, EncodeError> {
        Ok(Value::ctor(*self as u8, crate::value::HeapAddress::NULL))
    }
}

