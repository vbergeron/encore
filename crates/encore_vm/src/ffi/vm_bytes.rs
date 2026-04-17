//! `VmBytes`: a handle to a VM-heap byte string.

use crate::value::Value;
use crate::vm::Vm;

use super::decode::{DecodeError, ValueDecode};
use super::encode::{EncodeError, ValueEncode};

/// A handle to a VM-heap byte string. Carries only the `Value` tag-word;
/// actual bytes are read on demand through `&Vm` to avoid allocation.
#[derive(Clone, Copy, Debug)]
pub struct VmBytes(Value);

impl VmBytes {
    /// Non-owning lazy view of a Rust byte slice as an [`AsVmBytes`] writer —
    /// a *recipe* for a VM-heap byte string, encoded only when handed to the
    /// VM.
    ///
    /// Use this when the caller has Rust bytes and wants to pass them as a
    /// VM argument without allocating up front (no `&mut Vm` needed at
    /// construction time).
    ///
    /// ```ignore
    /// let digest: VmBytes = vm.call(funcs::HASH, (VmBytes::view(b"hello"),))?;
    /// ```
    pub fn view(data: &[u8]) -> AsVmBytes<'_> {
        AsVmBytes(data)
    }

    /// Allocate a fresh `VmBytes` on the VM heap from a Rust byte slice.
    ///
    /// This is the "Rust → VM" entry point, the parallel of
    /// [`VmList::build`](super::vm_list::VmList::build).
    pub fn build(vm: &mut Vm, data: &[u8]) -> Result<Self, EncodeError> {
        let value = vm.alloc_bytes(data).map_err(EncodeError::from)?;
        Ok(VmBytes(value))
    }

    /// Allocate an empty `VmBytes`.
    ///
    /// Unlike `VmList::nil`, this still touches the heap: every byte string
    /// (including zero-length) carries a header in the VM's value layout.
    pub fn empty(vm: &mut Vm) -> Result<Self, EncodeError> {
        Self::build(vm, &[])
    }

    pub fn len(&self, vm: &Vm) -> usize {
        vm.bytes_len(self.0)
    }

    pub fn is_empty(&self, vm: &Vm) -> bool {
        self.len(vm) == 0
    }

    pub fn get(&self, vm: &Vm, idx: usize) -> u8 {
        vm.bytes_read(self.0, idx)
    }

    /// Materialize the VM-heap bytes into a caller-supplied Rust buffer,
    /// returning the filled sub-slice.
    ///
    /// If `buf` is shorter than the VM-side string, only the leading bytes
    /// that fit are copied; any trailing capacity in `buf` is left untouched.
    /// If `buf` is longer, the returned sub-slice reflects the actual string
    /// length.
    ///
    /// This is the bytes counterpart of
    /// [`VmList::materialize`](super::vm_list::VmList::materialize).
    pub fn materialize<'b>(&self, vm: &Vm, buf: &'b mut [u8]) -> &'b [u8] {
        vm.bytes_slice(self.0, buf)
    }

    /// The underlying `Value` handle — useful for passing into raw VM calls
    /// that take `&[Value]`.
    pub fn as_value(&self) -> Value {
        self.0
    }
}

impl ValueDecode for VmBytes {
    fn decode(_vm: &Vm, value: Value) -> Result<Self, DecodeError> {
        if !value.is_bytes() {
            return Err(DecodeError::TypeMismatch {
                expected: "bytes",
                got: value.type_name(),
            });
        }
        Ok(VmBytes(value))
    }
}

/// Round-trip: a `VmBytes` is already a VM-heap value, so encoding it is
/// just handing back the wrapped `Value`. No allocation, no copy.
///
/// This makes it cheap to feed bytes returned from one VM call directly
/// into another.
impl ValueEncode for VmBytes {
    fn encode(&self, _vm: &mut Vm) -> Result<Value, EncodeError> {
        Ok(self.0)
    }
}

/// A *description* of a VM-heap byte string to build from a Rust slice.
///
/// The parallel of [`AsVmList`](super::vm_list::AsVmList) for bytes. Unlike
/// [`VmBytes`], which is a handle to bytes already on the VM heap,
/// `AsVmBytes` is purely a zero-cost wrapper around a `&[u8]`. The heap
/// allocation happens only when `encode` is called with a `&mut Vm`.
///
/// # Why it exists
/// This lets you describe byte-string arguments *without* needing a VM
/// reference in scope:
///
/// ```ignore
/// // No &mut Vm needed — just data.
/// let payload = AsVmBytes(b"hello");
///
/// // Allocation happens here.
/// let digest: VmBytes = vm.call(funcs::HASH, (payload,))?;
/// ```
///
/// Re-encoding the same `AsVmBytes` produces an independent VM allocation
/// each time; it's a recipe, not a handle.
pub struct AsVmBytes<'a>(pub &'a [u8]);

impl ValueEncode for AsVmBytes<'_> {
    fn encode(&self, vm: &mut Vm) -> Result<Value, EncodeError> {
        vm.alloc_bytes(self.0).map_err(EncodeError::from)
    }
}
