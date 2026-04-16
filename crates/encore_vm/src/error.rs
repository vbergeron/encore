use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternError(pub &'static str);

impl fmt::Display for ExternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmError {
    HeapOverflow,
    InvalidOpcode { opcode: u8, pc: u16 },
    MatchFail { tag: u8, pc: u16 },
    ByteRange { value: i32, pc: u16 },
    BadMagic,
    Truncated,
    UnregisteredExtern,
    Extern { error: ExternError, slot: u16, pc: u16 },
    TypeError { expected: &'static str, got: &'static str },
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::HeapOverflow => write!(f, "heap overflow"),
            VmError::InvalidOpcode { opcode, pc } =>
                write!(f, "invalid opcode 0x{opcode:02x} at pc=0x{pc:04x}"),
            VmError::MatchFail { tag, pc } =>
                write!(f, "match failure: no branch for tag {tag} at pc=0x{pc:04x}"),
            VmError::ByteRange { value, pc } =>
                write!(f, "byte range error: value {value} out of 0..255 at pc=0x{pc:04x}"),
            VmError::BadMagic => write!(f, "bad magic bytes (expected ENCR)"),
            VmError::Truncated => write!(f, "truncated program"),
            VmError::UnregisteredExtern => write!(f, "unregistered extern"),
            VmError::Extern { error, slot, pc } =>
                write!(f, "extern {slot} failed at pc=0x{pc:04x}: {error}"),
            VmError::TypeError { expected, got } =>
                write!(f, "type error: expected {expected}, got {got}"),
        }
    }
}
