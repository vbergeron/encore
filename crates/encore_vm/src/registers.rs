//! 256-slot register file with unchecked access for `Reg`-typed indices.
//!
//! `Reg` wraps a `u8`, so its range (0..=255) is exactly the valid index set
//! for a 256-element array. Indexing by `Reg` uses `get_unchecked` to skip
//! the bounds check that LLVM otherwise fails to elide through the
//! `u8 -> usize` conversion.

use crate::value::{Reg, Value};

const N_REGS: usize = 256;
const NULL_SENTINEL: Value = Value::function_const(0xFFFF);

pub(crate) struct Registers([Value; N_REGS]);

impl Registers {
    pub fn new() -> Self {
        let mut regs = [Value::int(0); N_REGS];
        regs[0xFF] = NULL_SENTINEL;
        Self(regs)
    }

    pub fn as_mut_slice(&mut self) -> &mut [Value] {
        &mut self.0
    }
}

impl core::ops::Index<Reg> for Registers {
    type Output = Value;
    #[inline(always)]
    fn index(&self, r: Reg) -> &Value {
        // SAFETY: Reg holds a u8, so r.raw() is in 0..=255, always in bounds
        // for a [Value; 256].
        unsafe { self.0.get_unchecked(r.raw() as usize) }
    }
}

impl core::ops::IndexMut<Reg> for Registers {
    #[inline(always)]
    fn index_mut(&mut self, r: Reg) -> &mut Value {
        // SAFETY: same as above — u8 cannot exceed 255.
        unsafe { self.0.get_unchecked_mut(r.raw() as usize) }
    }
}
