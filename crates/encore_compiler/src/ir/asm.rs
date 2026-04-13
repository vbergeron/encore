use super::prim::PrimOp;

pub type Tag = u8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Loc {
    Arg,
    Cont,
    NullCont,
    Local(u8),
    Capture(u8),
    Global(u8),
    SelfRef,
}

#[derive(Debug, PartialEq)]
pub struct Fun {
    pub captures: Vec<Loc>,
    pub body: Box<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct ContLam {
    pub captures: Vec<Loc>,
    pub body: Box<Expr>,
}

#[derive(Debug, PartialEq)]
pub struct Case {
    pub arity: u8,
    pub body: Expr,
}

#[derive(Debug, PartialEq)]
pub enum Val {
    Loc(Loc),
    ContLam(ContLam),
    Ctor(Tag, Vec<Loc>),
    Field(Loc, u8),
    Int(i32),
    Prim(PrimOp, Vec<Loc>),
    Extern(u16),
}

#[derive(Debug, PartialEq)]
pub enum Expr {
    Let(Val, Box<Expr>),
    Letrec(Fun, Box<Expr>),
    Encore(Loc, Loc, Loc),
    Match(Loc, Tag, Vec<Case>),
    Fin(Loc),
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
