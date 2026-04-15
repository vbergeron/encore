use super::prim::PrimOp;

pub type Tag = u8;
pub type Reg = u8;

pub const SELF: Reg = 0;
pub const CONT: Reg = 1;
pub const A1: Reg = 2;
pub const A2: Reg = 3;
pub const A3: Reg = 4;
pub const A4: Reg = 5;
pub const A5: Reg = 6;
pub const A6: Reg = 7;
pub const A7: Reg = 8;
pub const A8: Reg = 9;
pub const X01: Reg = 10;
pub const NULL: Reg = 0xFF;

#[derive(Debug, PartialEq)]
pub struct Fun {
    pub captures: Vec<Reg>,
    pub body: Box<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct ContLam {
    pub captures: Vec<Reg>,
    pub body: Box<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct Case {
    pub arity: u8,
    pub unpack_base: Reg,
    pub body: Expr,
}

#[derive(Debug, PartialEq)]
pub enum Val {
    Reg(Reg),
    Capture(u8),
    Global(u8),
    ContLam(ContLam),
    Ctor(Tag, Vec<Reg>),
    Field(Reg, u8),
    Int(i32),
    Bytes(Vec<u8>),
    Prim(PrimOp, Vec<Reg>),
    Extern(u16),
}

#[derive(Debug, PartialEq)]
pub enum Expr {
    Let(Reg, Val, Box<Expr>),
    Letrec(Reg, Fun, Box<Expr>),
    Encore(Reg, Vec<Reg>, Reg),
    Match(Reg, Tag, Vec<Case>),
    Fin(Reg),
}

#[derive(Debug)]
pub struct Define {
    pub global: u8,
    pub body: Expr,
}

#[derive(Debug)]
pub struct Module {
    pub defines: Vec<Define>,
}
