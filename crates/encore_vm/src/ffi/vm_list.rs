//! `VmList<T>`: a typed, lazy handle to a VM-heap `Nil | Cons(head, tail)` list.

use core::borrow::Borrow;
use core::marker::PhantomData;

use crate::builtins::{TAG_CONS, TAG_NIL};
use crate::value::{HeapAddress, Value};
use crate::vm::Vm;

use super::decode::{DecodeError, ValueDecode};
use super::encode::{EncodeError, ValueEncode};

/// A lazy handle to a `Nil | Cons(head, tail)` list in the VM heap.
///
/// Carries only the root `Value`; elements are read on demand through `&Vm`
/// without any allocation. The element type `T` is threaded through the
/// handle so callers don't have to turbofish each [`next`](VmList::next) call.
///
/// The `PhantomData<fn() -> T>` is purely a marker: `VmList<T>` only ever
/// produces `T` values, never stores them, so it remains `Copy` and thread-safe
/// regardless of `T`'s own bounds.
pub struct VmList<T>(Value, PhantomData<fn() -> T>);

impl<T> VmList<T> {
    /// Non-owning lazy view of a Rust slice as an [`AsVmList`] writer — a
    /// *recipe* for a VM-heap list, encoded only when handed to the VM.
    ///
    /// Use this when the caller has Rust data and wants to pass it as a list
    /// argument without first allocating on the VM heap (no `&mut Vm` needed
    /// at construction time).
    ///
    /// ```ignore
    /// let sorted: VmList<i32> = vm.call(funcs::SORT, (VmList::view(&[3, 1, 2]),))?;
    /// ```
    pub fn view<'a>(items: &'a [T]) -> AsVmList<'a, T> {
        AsVmList(items)
    }

    /// The empty list. Doesn't touch the heap — `Nil` is a nullary ctor.
    pub fn nil() -> Self {
        VmList(Value::ctor(TAG_NIL, HeapAddress::NULL), PhantomData)
    }

    pub fn is_nil(&self) -> bool {
        self.0.ctor_tag() == TAG_NIL
    }

    /// The underlying `Value` handle — useful for passing the list into
    /// raw VM calls that take `&[Value]`.
    pub fn as_value(&self) -> Value {
        self.0
    }

    /// Decode the head and return the tail, or `None` if the list is nil.
    pub fn next(&self, vm: &Vm) -> Option<(T, VmList<T>)>
    where
        T: ValueDecode,
    {
        if self.is_nil() { return None; }
        let head = T::decode(vm, vm.ctor_field(self.0, 0)).ok()?;
        let tail = VmList(vm.ctor_field(self.0, 1), PhantomData);
        Some((head, tail))
    }

    /// Materialize the VM-heap list into a caller-supplied Rust buffer,
    /// returning the filled sub-slice.
    ///
    /// Walks the cons-chain, decoding each head into successive slots of
    /// `buf`. Fails with [`DecodeError::BufferTooShort`] if the list has
    /// more elements than `buf` can hold. If `buf` is longer than the list,
    /// the returned slice reflects how many elements were actually written.
    ///
    /// This is the list counterpart of
    /// [`VmBytes::materialize`](super::vm_bytes::VmBytes::materialize).
    pub fn materialize<'b>(&self, vm: &Vm, buf: &'b mut [T]) -> Result<&'b [T], DecodeError>
    where
        T: ValueDecode,
    {
        let mut cursor = *self;
        let mut written = 0;
        while written < buf.len() {
            match cursor.next(vm) {
                Some((head, tail)) => {
                    buf[written] = head;
                    cursor = tail;
                    written += 1;
                }
                None => return Ok(&buf[..written]),
            }
        }
        if !cursor.is_nil() {
            return Err(DecodeError::BufferTooShort {
                needed: written + 1,
                provided: buf.len(),
            });
        }
        Ok(&buf[..written])
    }
}

impl<T: ValueEncode> VmList<T> {
    /// Build a `VmList<T>` on the VM heap from a Rust iterable.
    ///
    /// Each element is encoded with `T::encode`, then `Cons`-cells are
    /// allocated tail-first so the resulting list has the same order as
    /// the input iterator.
    ///
    /// Requires `DoubleEndedIterator` to avoid a temporary buffer in `no_std`.
    /// For `Vec`, `&[T]`, arrays, and `Range` this is already free.
    pub fn build<I>(vm: &mut Vm, items: I) -> Result<Self, EncodeError>
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: DoubleEndedIterator,
    {
        Ok(VmList(Self::build_cons_chain::<T, _>(vm, items)?, PhantomData))
    }

    /// Cons an element onto the front of an existing list.
    ///
    /// This is the single-step form of [`build`](Self::build); useful when
    /// assembling a list from a fold or a stream where you already have a
    /// running tail.
    pub fn cons(vm: &mut Vm, head: T, tail: VmList<T>) -> Result<Self, EncodeError> {
        let head = head.encode(vm)?;
        let cell = vm
            .alloc_ctor(TAG_CONS, &[head, tail.0])
            .map_err(EncodeError::from)?;
        Ok(VmList(cell, PhantomData))
    }

    /// Allocate a `Nil | Cons(head, tail)` chain on the VM heap from any
    /// double-ended iterator whose items can be borrowed as `T`.
    ///
    /// The `Borrow<T>` bound is what lets this serve both [`build`](Self::build)
    /// (owned `T`s — `T: Borrow<T>` is blanket) and [`AsVmList::encode`]
    /// (`&T`s from a slice iter — `&T: Borrow<T>` is also blanket).
    ///
    /// Returns the raw `Value` rather than a `VmList<T>` so callers can either
    /// wrap it (when constructing a handle) or feed it straight into another
    /// VM call (when used as an encoded argument).
    fn build_cons_chain<B, I>(vm: &mut Vm, items: I) -> Result<Value, EncodeError>
    where
        B: Borrow<T>,
        I: IntoIterator<Item = B>,
        I::IntoIter: DoubleEndedIterator,
    {
        let mut tail = Value::ctor(TAG_NIL, HeapAddress::NULL);
        for item in items.into_iter().rev() {
            let head = item.borrow().encode(vm)?;
            tail = vm
                .alloc_ctor(TAG_CONS, &[head, tail])
                .map_err(EncodeError::from)?;
        }
        Ok(tail)
    }
}

/// An iterator over a `VmList<T>` that borrows the `Vm` for element decoding.
///
/// Produced by [`VmList::iter`]. Implements [`Iterator`] so it works in
/// `for` loops and with all iterator adaptors.
///
/// ```ignore
/// for effect in effects.iter(&vm) {
///     match effect { ... }
/// }
/// ```
pub struct VmIter<'vm, 'heap, T> {
    vm: &'vm Vm<'heap>,
    cursor: VmList<T>,
}

impl<'vm, 'heap, T: ValueDecode> Iterator for VmIter<'vm, 'heap, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let (head, tail) = self.cursor.next(self.vm)?;
        self.cursor = tail;
        Some(head)
    }
}

impl<T: ValueDecode> VmList<T> {
    /// Return a borrowing iterator over this list.
    ///
    /// The iterator holds `&'vm Vm` so it can decode each element lazily
    /// without any allocation.
    pub fn iter<'vm, 'heap>(&self, vm: &'vm Vm<'heap>) -> VmIter<'vm, 'heap, T> {
        VmIter { vm, cursor: *self }
    }
}

// Manual impls: `T` doesn't need to be `Clone`/`Copy`/`Debug` — the handle
// only wraps a `Value` plus a zero-sized marker.
impl<T> Clone for VmList<T> {
    fn clone(&self) -> Self { *self }
}
impl<T> Copy for VmList<T> {}

impl<T> core::fmt::Debug for VmList<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("VmList").field(&self.0).finish()
    }
}

impl<T> ValueDecode for VmList<T> {
    fn decode(_vm: &Vm, value: Value) -> Result<Self, DecodeError> {
        if !value.is_ctor() {
            return Err(DecodeError::TypeMismatch {
                expected: "List",
                got: value.type_name(),
            });
        }
        Ok(VmList(value, PhantomData))
    }
}

/// Round-trip: a `VmList<T>` is already a VM-heap value, so encoding it is
/// just handing back the wrapped `Value`. No allocation, no traversal.
///
/// This makes it cheap to feed a list returned from one VM call directly
/// into another.
impl<T> ValueEncode for VmList<T> {
    fn encode(&self, _vm: &mut Vm) -> Result<Value, EncodeError> {
        Ok(self.0)
    }
}

/// A *description* of a VM-heap list to build from a Rust slice.
///
/// Unlike `VmList<T>`, which is a handle to data that already lives on the
/// VM heap, `AsVmList` is purely a zero-cost wrapper around a Rust slice. It
/// does nothing on construction — the cons-chain is allocated later, when
/// `encode` is called with an actual `&mut Vm`.
///
/// # Why it exists
/// This lets you describe list arguments *without* needing a VM reference:
///
/// ```ignore
/// // No &mut Vm in scope yet — just data.
/// let request = AsVmList(&[3, 1, 2]);
///
/// // Encoding (and the allocation) happens here.
/// let sorted: VmList<i32> = vm.call(funcs::SORT, (request,))?;
/// ```
///
/// Re-encoding the same `AsVmList` produces an independent VM allocation each
/// time; it's a recipe, not a handle.
pub struct AsVmList<'a, T>(pub &'a [T]);

impl<T: ValueEncode> ValueEncode for AsVmList<'_, T> {
    fn encode(&self, vm: &mut Vm) -> Result<Value, EncodeError> {
        VmList::<T>::build_cons_chain::<&T, _>(vm, self.0.iter())
    }
}
