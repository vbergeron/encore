#[derive(Debug, Clone, Copy)]
pub enum VmError {
    HeapOverflow,
    StackOverflow,
    InvalidOpcode(u8),
}
