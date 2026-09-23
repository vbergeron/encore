#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntOp {
    Add,
    Sub,
    Mul,
    Eq,
    Lt,
    Byte,
    /// Truncating division, `x / 0 = 0` (Rocq `Nat.div`).
    Div,
    /// Truncating remainder, `x mod 0 = x` (Rocq `Nat.modulo`).
    Mod,
    /// Truncated subtraction, `max(a - b, 0)` (Rocq `Nat.sub`).
    SubSat,
    Le,
    And,
    Or,
    Xor,
    Shl,
    /// Logical right shift of the 24-bit payload.
    Shr,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BytesOp {
    Len,
    Get,
    Concat,
    Slice,
    Eq,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimOp {
    Int(IntOp),
    Bytes(BytesOp),
}
