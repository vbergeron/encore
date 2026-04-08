use crate::error::VmError;
use crate::value::Value;

pub const MAGIC: [u8; 4] = *b"ENCR";

/// Binary format:
///   [magic: 4 bytes "ENCR"]
///   [n_arities: u16 LE]
///   [n_globals: u16 LE]
///   [code_len: u16 LE]
///   [arity_table: n_arities bytes]
///   [globals: n_globals * 4 bytes, each u32 LE]
///   [code: code_len bytes]
#[derive(Debug)]
pub struct Program<'a> {
    pub arity_table: &'a [u8],
    pub code: &'a [u8],
    globals_raw: &'a [u8],
    n_globals: usize,
}

const HEADER: usize = 4 + 6;

impl<'a> Program<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, VmError> {
        if bytes.len() < HEADER { return Err(VmError::Truncated); }
        if bytes[0..4] != MAGIC { return Err(VmError::BadMagic); }

        let n_arities = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        let n_globals = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        let code_len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;

        let expected = HEADER + n_arities + n_globals * 4 + code_len;
        if bytes.len() < expected { return Err(VmError::Truncated); }

        let arity_start = HEADER;
        let globals_start = arity_start + n_arities;
        let code_start = globals_start + n_globals * 4;

        Ok(Self {
            arity_table: &bytes[arity_start..globals_start],
            globals_raw: &bytes[globals_start..code_start],
            code: &bytes[code_start..code_start + code_len],
            n_globals,
        })
    }

    pub fn n_globals(&self) -> usize { self.n_globals }

    pub fn global(&self, idx: usize) -> Value {
        let off = idx * 4;
        Value::from_u32(u32::from_le_bytes([
            self.globals_raw[off],
            self.globals_raw[off + 1],
            self.globals_raw[off + 2],
            self.globals_raw[off + 3],
        ]))
    }

    pub fn load_globals(&self, buf: &mut [Value]) {
        for i in 0..self.n_globals {
            buf[i] = self.global(i);
        }
    }
}
