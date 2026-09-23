use std::fmt;

/// A program exceeds one of the hard limits of the bytecode format or VM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// More defines than the `u16` global table can index.
    TooManyGlobals { count: usize, max: usize },
    /// Code does not fit in the `u16` code address space.
    CodeTooLarge { size: usize, max: usize },
    /// A byte-string literal longer than the `u8` length of `BYTES`.
    BytesLiteralTooLong { len: usize, max: usize },
    /// A closure captures more values than a heap object can hold.
    TooManyCaptures { count: usize, max: usize },
    /// A constructor has more fields than a heap object can hold.
    CtorTooWide { tag: u8, arity: usize, max: usize },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::TooManyGlobals { count, max } =>
                write!(f, "too many top-level defines: {count} (max {max})"),
            CompileError::CodeTooLarge { size, max } =>
                write!(f, "bytecode too large: {size} bytes (max {max}, code addresses are u16)"),
            CompileError::BytesLiteralTooLong { len, max } =>
                write!(f, "byte-string literal too long: {len} bytes (max {max})"),
            CompileError::TooManyCaptures { count, max } =>
                write!(f, "closure captures too many values: {count} (max {max})"),
            CompileError::CtorTooWide { tag, arity, max } =>
                write!(f, "constructor tag {tag} has too many fields: {arity} (max {max})"),
        }
    }
}

impl std::error::Error for CompileError {}
