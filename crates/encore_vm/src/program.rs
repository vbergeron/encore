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
///
/// Optional metadata (appended after code):
///   Section 1 - constructor names:
///     [n_ctors: u16 LE]
///     For each: [tag: u8] [name_len: u8] [name: name_len bytes, UTF-8]
///   Section 2 - global/define names:
///     [n_globals: u16 LE]
///     For each: [idx: u8] [name_len: u8] [name: name_len bytes, UTF-8]
#[derive(Debug)]
pub struct Program<'a> {
    pub arity_table: &'a [u8],
    pub code: &'a [u8],
    globals_raw: &'a [u8],
    n_globals: usize,
    metadata: &'a [u8],
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
        let code_end = code_start + code_len;

        Ok(Self {
            arity_table: &bytes[arity_start..globals_start],
            globals_raw: &bytes[globals_start..code_start],
            code: &bytes[code_start..code_end],
            n_globals,
            metadata: &bytes[code_end..],
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

    pub fn has_metadata(&self) -> bool {
        self.metadata.len() >= 2
    }

    pub fn ctor_names(&self) -> NameEntryIter<'a> {
        parse_name_section(self.metadata)
    }

    pub fn global_names(&self) -> NameEntryIter<'a> {
        let rest = skip_name_section(self.metadata);
        parse_name_section(rest)
    }
}

fn parse_name_section<'a>(data: &'a [u8]) -> NameEntryIter<'a> {
    if data.len() < 2 {
        return NameEntryIter { data, pos: 0, remaining: 0 };
    }
    let n = u16::from_le_bytes([data[0], data[1]]) as usize;
    NameEntryIter { data, pos: 2, remaining: n }
}

fn skip_name_section<'a>(data: &'a [u8]) -> &'a [u8] {
    if data.len() < 2 { return &[]; }
    let n = u16::from_le_bytes([data[0], data[1]]) as usize;
    let mut pos = 2;
    for _ in 0..n {
        if pos + 2 > data.len() { return &[]; }
        let name_len = data[pos + 1] as usize;
        pos += 2 + name_len;
        if pos > data.len() { return &[]; }
    }
    &data[pos..]
}

pub struct NameEntryIter<'a> {
    data: &'a [u8],
    pos: usize,
    remaining: usize,
}

impl<'a> Iterator for NameEntryIter<'a> {
    type Item = (u8, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 { return None; }
        if self.pos + 2 > self.data.len() { return None; }
        let idx = self.data[self.pos];
        let name_len = self.data[self.pos + 1] as usize;
        self.pos += 2;
        if self.pos + name_len > self.data.len() { return None; }
        let name = core::str::from_utf8(&self.data[self.pos..self.pos + name_len]).ok()?;
        self.pos += name_len;
        self.remaining -= 1;
        Some((idx, name))
    }
}
