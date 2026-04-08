use crate::value::CodeAddress;

pub struct Code<'a> {
    bytes: &'a [u8],
    pc: usize,
}

impl<'a> Code<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pc: 0 }
    }

    pub fn read_u8(&mut self) -> u8 {
        let b = self.bytes[self.pc];
        self.pc += 1;
        b
    }

    pub fn read_address(&mut self) -> CodeAddress {
        let lo = self.bytes[self.pc] as u16;
        let hi = self.bytes[self.pc + 1] as u16;
        self.pc += 2;
        CodeAddress::new(lo | (hi << 8))
    }

    pub fn read_address_at(&self, pos: usize) -> CodeAddress {
        let lo = self.bytes[pos] as u16;
        let hi = self.bytes[pos + 1] as u16;
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
