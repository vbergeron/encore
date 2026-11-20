//! Textual (S-expression) representations of every IR.
//!
//! Each IR module gets `to_text()` / `from_text()`; printing then parsing
//! yields a structurally identical module. See `IR.md` for the grammar.

pub mod sexp;
mod ds;
mod dsi;
mod cps;
mod asm;

pub use sexp::TextError;

use crate::ir::prim::{BytesOp, IntOp, PrimOp};
use sexp::{Node, Sexp};

const PRIMS: &[PrimOp] = &[
    PrimOp::Int(IntOp::Add),
    PrimOp::Int(IntOp::Sub),
    PrimOp::Int(IntOp::Mul),
    PrimOp::Int(IntOp::Eq),
    PrimOp::Int(IntOp::Lt),
    PrimOp::Int(IntOp::Byte),
    PrimOp::Int(IntOp::Div),
    PrimOp::Int(IntOp::Mod),
    PrimOp::Int(IntOp::SubSat),
    PrimOp::Int(IntOp::Le),
    PrimOp::Int(IntOp::And),
    PrimOp::Int(IntOp::Or),
    PrimOp::Int(IntOp::Xor),
    PrimOp::Int(IntOp::Shl),
    PrimOp::Int(IntOp::Shr),
    PrimOp::Bytes(BytesOp::Len),
    PrimOp::Bytes(BytesOp::Get),
    PrimOp::Bytes(BytesOp::Concat),
    PrimOp::Bytes(BytesOp::Slice),
    PrimOp::Bytes(BytesOp::Eq),
];

fn prim_sexp(op: PrimOp) -> Sexp {
    sexp::sym(op.to_string())
}

fn parse_prim(node: &Node) -> Result<PrimOp, TextError> {
    let name = node.as_sym()?;
    PRIMS.iter()
        .copied()
        .find(|op| op.to_string() == name)
        .map_or_else(|| node.err(format!("unknown primitive `{name}`")), Ok)
}

fn parse_module<D>(
    src: &str,
    define: impl Fn(&Node, &[Node]) -> Result<D, TextError>,
) -> Result<Vec<D>, TextError> {
    sexp::parse(src)?
        .iter()
        .map(|node| match node.as_form() {
            Some(("define", args)) => define(node, args),
            _ => node.err(format!("expected `(define ...)`, found {}", node.describe())),
        })
        .collect()
}
