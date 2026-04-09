use super::prim::PrimOp;

pub type Tag = u8;

#[derive(Debug, Clone, Copy)]
pub enum Loc {
    Arg,
    Cont,
    Local(u8),
    Capture(u8),
    Global(u8),
    SelfRef,
}

pub struct Fun {
    pub captures: Vec<Loc>,
    pub body: Box<Expr>,
}

pub struct ContLam {
    pub captures: Vec<Loc>,
    pub body: Box<Expr>,
}

pub struct Case {
    pub arity: u8,
    pub body: Expr,
}

pub enum Val {
    Loc(Loc),
    ContLam(ContLam),
    Ctor(Tag, Vec<Loc>),
    Field(Loc, u8),
    Int(i32),
    Prim(PrimOp, Vec<Loc>),
}

pub enum Expr {
    Let(Val, Box<Expr>),
    Letrec(Fun, Box<Expr>),
    Encore(Loc, Loc, Loc),
    Return(Loc, Loc),
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
