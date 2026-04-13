#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmError {
    HeapOverflow,
    StackOverflow,
    InvalidOpcode(u8),
    NotRegistered(u16),
    MatchFail,
    BadMagic,
    Truncated,
}
