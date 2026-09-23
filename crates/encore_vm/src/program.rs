use crate::error::VmError;
use crate::value::CodeAddress;

pub const MAGIC: [u8; 4] = *b"ENCR";

/// Binary format:
///   [magic: 4 bytes "ENCR"]
///   [n_arities: u16 LE]
///   [n_globals: u16 LE]
///   [code_len: u16 LE]
///   [arity_table: n_arities bytes]
///   [globals: n_globals * 2 bytes, each u16 LE code offset]
///   [code: code_len bytes]
///
/// Optional metadata (appended after code):
///   Section 1 - constructor names:
///     [n_ctors: u16 LE]
///     For each: [tag: u8] [name_len: u8] [name: name_len bytes, UTF-8]
///   Section 2 - global/define names:
///     [n_globals: u16 LE]
///     For each: [idx: u16 LE] [name_len: u8] [name: name_len bytes, UTF-8]
///
/// `parse` checks the header only, so tools such as the disassembler can
/// still open malformed code. [`Vm::load`](crate::vm::Vm::load) runs the full
/// [`validate`](Self::validate) pass before executing anything.
#[derive(Debug)]
pub struct Program<'a> {
    pub arity_table: &'a [u8],
    pub code: &'a [u8],
    globals: Globals<'a>,
    metadata: &'a [u8],
}

/// Global entry points: either given directly, or the raw `u16 LE` table of
/// a parsed binary. Neither form has a capacity limit.
#[derive(Debug)]
enum Globals<'a> {
    Slice(&'a [CodeAddress]),
    Raw(&'a [u8]),
}

const HEADER: usize = 4 + 6;

impl<'a> Program<'a> {
    pub fn new(code: &'a [u8], arity_table: &'a [u8], globals: &'a [CodeAddress]) -> Self {
        Self {
            arity_table,
            code,
            globals: Globals::Slice(globals),
            metadata: &[],
        }
    }

    pub fn parse(bytes: &'a [u8]) -> Result<Self, VmError> {
        if bytes.len() < HEADER { return Err(VmError::Truncated); }
        if bytes[0..4] != MAGIC { return Err(VmError::BadMagic); }

        let n_arities = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        let n_globals = u16::from_le_bytes([bytes[6], bytes[7]]) as usize;
        let code_len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;

        let expected = HEADER + n_arities + n_globals * 2 + code_len;
        if bytes.len() < expected { return Err(VmError::Truncated); }

        let arity_start = HEADER;
        let globals_start = arity_start + n_arities;
        let code_start = globals_start + n_globals * 2;
        let code_end = code_start + code_len;

        Ok(Self {
            arity_table: &bytes[arity_start..globals_start],
            code: &bytes[code_start..code_end],
            globals: Globals::Raw(&bytes[globals_start..code_start]),
            metadata: &bytes[code_end..],
        })
    }

    pub fn n_globals(&self) -> usize {
        match self.globals {
            Globals::Slice(s) => s.len(),
            Globals::Raw(raw) => raw.len() / 2,
        }
    }

    pub fn global(&self, idx: usize) -> CodeAddress {
        match self.globals {
            Globals::Slice(s) => s[idx],
            Globals::Raw(raw) => CodeAddress::new(u16::from_le_bytes([raw[idx * 2], raw[idx * 2 + 1]])),
        }
    }

    /// Check that the code is safe to execute; see [`crate::validate`].
    pub fn validate(&self) -> Result<(), VmError> {
        crate::validate::validate(self)
    }

    pub fn has_metadata(&self) -> bool {
        self.metadata.len() >= 2
    }

    /// Constructor names: `(tag, name)`.
    pub fn ctor_names(&self) -> impl Iterator<Item = (u8, &'a str)> {
        parse_name_section(self.metadata, CTOR_IDX_WIDTH).map(|(tag, name)| (tag as u8, name))
    }

    /// Global/define names: `(global index, name)`.
    pub fn global_names(&self) -> NameEntryIter<'a> {
        let rest = skip_name_section(self.metadata, CTOR_IDX_WIDTH);
        parse_name_section(rest, GLOBAL_IDX_WIDTH)
    }
}

/// Width in bytes of the index field of a name entry.
const CTOR_IDX_WIDTH: usize = 1;
const GLOBAL_IDX_WIDTH: usize = 2;

fn parse_name_section<'a>(data: &'a [u8], idx_width: usize) -> NameEntryIter<'a> {
    if data.len() < 2 {
        return NameEntryIter { data, pos: 0, remaining: 0, idx_width };
    }
    let n = u16::from_le_bytes([data[0], data[1]]) as usize;
    NameEntryIter { data, pos: 2, remaining: n, idx_width }
}

fn skip_name_section<'a>(data: &'a [u8], idx_width: usize) -> &'a [u8] {
    if data.len() < 2 { return &[]; }
    let n = u16::from_le_bytes([data[0], data[1]]) as usize;
    let mut pos = 2;
    for _ in 0..n {
        if pos + idx_width + 1 > data.len() { return &[]; }
        let name_len = data[pos + idx_width] as usize;
        pos += idx_width + 1 + name_len;
        if pos > data.len() { return &[]; }
    }
    &data[pos..]
}

pub struct NameEntryIter<'a> {
    data: &'a [u8],
    pos: usize,
    remaining: usize,
    idx_width: usize,
}

impl<'a> Iterator for NameEntryIter<'a> {
    type Item = (u16, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 { return None; }
        if self.pos + self.idx_width + 1 > self.data.len() { return None; }
        let idx = if self.idx_width == 2 {
            u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]])
        } else {
            self.data[self.pos] as u16
        };
        let name_len = self.data[self.pos + self.idx_width] as usize;
        self.pos += self.idx_width + 1;
        if self.pos + name_len > self.data.len() { return None; }
        let name = core::str::from_utf8(&self.data[self.pos..self.pos + name_len]).ok()?;
        self.pos += name_len;
        self.remaining -= 1;
        Some((idx, name))
    }
}
