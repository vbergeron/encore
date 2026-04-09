use super::prim::PrimOp;

pub type Name = String;
pub type Tag = u8;

#[derive(Clone)]
pub enum Expr {
    Var(Name),
    Lam(Name, Box<Expr>),
    App(Box<Expr>, Box<Expr>),
    Let(Name, Box<Expr>, Box<Expr>),
    Letrec(Name, Name, Box<Expr>, Box<Expr>),
    Ctor(Tag, Vec<Expr>),
    Field(Box<Expr>, u8),
    Match(Box<Expr>, Tag, Vec<Case>),
    Int(i32),
    Prim(PrimOp, Vec<Expr>),
}

#[derive(Clone)]
pub struct Case {
    pub binds: Vec<Name>,
    pub body: Expr,
}

pub struct Define {
    pub name: Name,
    pub body: Expr,
}

pub struct Module {
    pub defines: Vec<Define>,
}
