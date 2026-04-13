use crate::error::VmError;
use crate::value::CodeAddress;

pub const MAGIC: [u8; 4] = *b"ENCR";

/// Binary format:
///   [magic: 4 bytes "ENCR"]
///   [n_arities: u16 LE]
///   [n_globals: u16 LE]
///   [code_len: u16 LE]
///   [arity_table: n_arities bytes]
///   [globals: n_globals * 3 bytes, each u16 LE code offset + u8 stack_delta]
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
    globals: [CodeAddress; 64],
    global_sds: [u8; 64],
    n_globals: u8,
    metadata: &'a [u8],
}

const HEADER: usize = 4 + 6;

impl<'a> Program<'a> {
    pub fn new(code: &'a [u8], arity_table: &'a [u8], globals: &[CodeAddress]) -> Self {
        let mut arr = [CodeAddress::new(0); 64];
        let n = globals.len().min(64);
        arr[..n].copy_from_slice(&globals[..n]);
        Self {
            arity_table,
            code,
            globals: arr,
            global_sds: [0; 64],
            n_globals: n as u8,
            metadata: &[],
        }
    }

    pub fn with_sds(code: &'a [u8], arity_table: &'a [u8], globals: &[CodeAddress], sds: &[u8]) -> Self {
        let mut arr = [CodeAddress::new(0); 64];
        let mut sd_arr = [0u8; 64];
        let n = globals.len().min(64);
        arr[..n].copy_from_slice(&globals[..n]);
        let sd_n = sds.len().min(n);
        sd_arr[..sd_n].copy_from_slice(&sds[..sd_n]);
        Self {
            arity_table,
            code,
            globals: arr,
            global_sds: sd_arr,
            n_globals: n as u8,
            metadata: &[],
        }
    }

    pub fn parse(bytes: &'a [u8]) -> Result<Self, VmError> {
        if bytes.len() < HEADER { return Err(VmError::Truncated); }
        if bytes[0..4] != MAGIC { return Err(VmError::BadMagic); }

        let n_arities = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        let n_globals = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        let code_len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;

        let expected = HEADER + n_arities + n_globals * 3 + code_len;
        if bytes.len() < expected { return Err(VmError::Truncated); }

        let arity_start = HEADER;
        let globals_start = arity_start + n_arities;
        let code_start = globals_start + n_globals * 3;
        let code_end = code_start + code_len;

        let mut globals = [CodeAddress::new(0); 64];
        let mut global_sds = [0u8; 64];
        for i in 0..n_globals {
            let off = globals_start + i * 3;
            let raw = u16::from_le_bytes([bytes[off], bytes[off + 1]]);
            globals[i] = CodeAddress::new(raw);
            global_sds[i] = bytes[off + 2];
        }

        Ok(Self {
            arity_table: &bytes[arity_start..globals_start],
            code: &bytes[code_start..code_end],
            globals,
            global_sds,
            n_globals: n_globals as u8,
            metadata: &bytes[code_end..],
        })
    }

    pub fn n_globals(&self) -> usize { self.n_globals as usize }

    pub fn global(&self, idx: usize) -> CodeAddress {
        self.globals[idx]
    }

    pub fn global_sd(&self, idx: usize) -> u8 {
        self.global_sds[idx]
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
