#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternError(pub &'static str);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    HeapOverflow,
    InvalidOpcode(u8),
    MatchFail,
    ByteRange(i32),
    BadMagic,
    Truncated,
    UnregisteredExtern,
    Extern(ExternError),
}
