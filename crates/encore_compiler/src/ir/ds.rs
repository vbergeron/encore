use super::prim::PrimOp;

pub type Name = String;
pub type Tag = u8;

#[derive(Clone)]
pub enum Expr {
    Var(Name),
    Lambda(Vec<Name>, Box<Expr>),
    Apply(Box<Expr>, Vec<Expr>),
    Let(Name, Box<Expr>, Box<Expr>),
    Letrec(Name, Name, Box<Expr>, Box<Expr>),
    Ctor(Tag, Vec<Expr>),
    Field(Box<Expr>, u8),
    Match(Box<Expr>, Tag, Vec<Case>),
    Int(i32),
    Bytes(Vec<u8>),
    Prim(PrimOp, Vec<Expr>),
    Extern(u16),
}

#[derive(Clone)]
pub struct Case {
    pub binds: Vec<Name>,
    pub body: Expr,
}

#[derive(Clone)]
pub struct Define {
    pub name: Name,
    pub body: Expr,
}

#[derive(Clone)]
pub struct Module {
    pub defines: Vec<Define>,
}
