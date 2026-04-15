#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmError {
    HeapOverflow,
    InvalidOpcode(u8),
    MatchFail,
    ByteRange(i32),
    BadMagic,
    Truncated,
}
