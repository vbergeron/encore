//! Cursor over the code stream, read without bounds checks.
//!
//! SAFETY: a `Code` is only built over bytes that passed
//! [`validate`](crate::validate), and the interpreter only moves `pc` by
//! decoding one instruction at a time from a boundary or by jumping to a
//! validated target (static targets are checked at load, dynamic `ENCORE`
//! targets by `Vm::resolve_code_ptr`). Validation guarantees every operand of
//! an instruction starting at a boundary lies inside the code and that no
//! instruction falls through past its end, so each unchecked read below is
//! in bounds.

use crate::value::{CodeAddress, Reg};

pub(crate) struct Code<'a> {
    bytes: &'a [u8],
    pc: usize,
}

impl<'a> Code<'a> {
    /// A cursor over no code. Nothing can execute: every call target is
    /// out of range.
    pub fn empty() -> Self {
        Self { bytes: &[], pc: 0 }
    }

    /// # Safety
    /// `bytes` must be the code of a program that passed validation.
    pub unsafe fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pc: 0 }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn read_u8(&mut self) -> u8 {
        let b = unsafe { *self.bytes.get_unchecked(self.pc) };
        self.pc += 1;
        b
    }

    pub fn read_u16(&mut self) -> u16 {
        let lo = unsafe { *self.bytes.get_unchecked(self.pc) } as u16;
        let hi = unsafe { *self.bytes.get_unchecked(self.pc + 1) } as u16;
        self.pc += 2;
        lo | (hi << 8)
    }

    pub fn read_u24(&mut self) -> u32 {
        let b0 = unsafe { *self.bytes.get_unchecked(self.pc) } as u32;
        let b1 = unsafe { *self.bytes.get_unchecked(self.pc + 1) } as u32;
        let b2 = unsafe { *self.bytes.get_unchecked(self.pc + 2) } as u32;
        self.pc += 3;
        b0 | (b1 << 8) | (b2 << 16)
    }

    pub fn read_reg(&mut self) -> Reg {
        Reg::new(self.read_u8())
    }

    pub fn read_address(&mut self) -> CodeAddress {
        CodeAddress::new(self.read_u16())
    }

    pub fn read_address_at(&self, pos: usize) -> CodeAddress {
        let lo = unsafe { *self.bytes.get_unchecked(pos) } as u16;
        let hi = unsafe { *self.bytes.get_unchecked(pos + 1) } as u16;
        CodeAddress::new(lo | (hi << 8))
    }

    pub fn jump(&mut self, target: CodeAddress) {
        self.pc = target.raw() as usize;
    }

    pub fn skip(&mut self, n: usize) {
        self.pc += n;
    }

    pub fn pc(&self) -> usize {
        self.pc
    }
}
