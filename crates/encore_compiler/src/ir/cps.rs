use super::prim::PrimOp;

pub type Name = String;
pub type Tag = u8;

#[derive(Debug, Clone, PartialEq)]
pub struct Fun {
    pub args: Vec<Name>,
    pub cont: Name,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cont {
    pub param: Name,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub binds: Vec<Name>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Var(Name),
    Cont(Cont),
    NullCont,
    Ctor(Tag, Vec<Name>),
    Field(Name, u8),
    Int(i32),
    Prim(PrimOp, Vec<Name>),
    Extern(u16),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Let(Name, Val, Box<Expr>),
    Letrec(Name, Fun, Box<Expr>),
    Encore(Name, Vec<Name>, Name),
    Match(Name, Tag, Vec<Case>),
    Fin(Name),
}

pub struct Define {
    pub name: Name,
    pub body: Expr,
}

pub struct Module {
    pub defines: Vec<Define>,
}
