use super::prim::PrimOp;

pub type Tag = u8;

#[derive(Debug, Clone, Copy)]
pub enum Loc {
    Arg,
    Local(u8),
    Capture(u8),
    Global(u8),
    SelfRef,
}

pub struct Lambda {
    pub captures: Vec<Loc>,
    pub body: Box<Expr>,
}

pub struct Case {
    pub arity: u8,
    pub body: Expr,
}

pub enum Val {
    Loc(Loc),
    Lambda(Lambda),
    Ctor(Tag, Vec<Loc>),
    Field(Loc, u8),
    Int(i32),
    Prim(PrimOp, Vec<Loc>),
}

pub enum Expr {
    Let(Val, Box<Expr>),
    Letrec(Lambda, Box<Expr>),
    App(Loc, Loc),
    Match(Loc, Tag, Vec<Case>),
    Fin(Loc),
}

pub struct Define {
    pub global: u8,
    pub body: Expr,
}

pub struct Module {
    pub defines: Vec<Define>,
}
