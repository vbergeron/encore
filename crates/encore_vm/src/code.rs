use crate::value::{CodeAddress, Reg};

pub struct Code<'a> {
    bytes: &'a [u8],
    pc: usize,
}

impl<'a> Code<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pc: 0 }
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
