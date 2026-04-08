#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmError {
    HeapOverflow,
    StackOverflow,
    InvalidOpcode(u8),
    BadMagic,
    Truncated,
}
