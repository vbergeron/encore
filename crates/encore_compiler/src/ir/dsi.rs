use super::prim::PrimOp;

pub type Tag = u8;
pub type Index = usize;

pub enum Expr {
    Var(Index),
    Lambda(usize, Box<Expr>),
    Apply(Box<Expr>, Vec<Expr>),
    Let(Box<Expr>, Box<Expr>),
    Letrec(Box<Expr>, Box<Expr>),
    Ctor(Tag, Vec<Expr>),
    Field(Box<Expr>, u8),
    Match(Box<Expr>, Tag, Vec<Case>),
    Int(i32),
    Bytes(Vec<u8>),
    Prim(PrimOp, Vec<Expr>),
    Extern(u16),
}

pub struct Case {
    pub arity: usize,
    pub body: Expr,
}

pub struct Define {
    pub name: String,
    pub body: Expr,
}

pub struct Module {
    pub defines: Vec<Define>,
}
