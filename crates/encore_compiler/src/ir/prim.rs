#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntOp {
    Add,
    Sub,
    Mul,
    Eq,
    Lt,
    Byte,
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
