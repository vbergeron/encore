use core::fmt;
use crate::ffi;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternError {
    Encode(ffi::EncodeError),
    Decode(ffi::DecodeError),
    Custom(&'static str),
    Nested(&'static str),
    Unregistered,
}

impl From<ffi::EncodeError> for ExternError {
    fn from(err: ffi::EncodeError) -> Self {
        Self::Encode(err)
    }
}

impl From<ffi::DecodeError> for ExternError {
    fn from(err: ffi::DecodeError) -> Self {
        Self::Decode(err)
    }
}

impl From<VmError> for ExternError {
    fn from(err: VmError) -> Self {
        Self::Nested(err.as_static_str())
    }
}

impl fmt::Display for ExternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(err) => write!(f, "encode error: {err:?}"),
            Self::Decode(err) => write!(f, "decode error: {err:?}"),
            Self::Custom(msg) => write!(f, "{msg}"),
            Self::Nested(msg) => write!(f, "nested vm error: {msg}"),
            Self::Unregistered => write!(f, "unregistered extern"),
        }
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
    Extern { error: ExternError, slot: u16, pc: u16 },
    TypeError { expected: &'static str, got: &'static str },
}

impl VmError {
    pub fn as_static_str(&self) -> &'static str {
        match self {
            VmError::HeapOverflow => "heap overflow",
            VmError::InvalidOpcode { .. } => "invalid opcode",
            VmError::MatchFail { .. } => "match failure",
            VmError::ByteRange { .. } => "byte range error",
            VmError::BadMagic => "bad magic",
            VmError::Truncated => "truncated program",
            VmError::Extern { .. } => "extern call failed",
            VmError::TypeError { .. } => "type error",
        }
    }
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
            VmError::Extern { error, slot, pc } =>
                write!(f, "extern {slot} failed at pc=0x{pc:04x}: {error}"),
            VmError::TypeError { expected, got } =>
                write!(f, "type error: expected {expected}, got {got}"),
        }
    }
}
