use encore_compiler::ir::prim::PrimOp;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Var(String),
    Int(i32),
    Bytes(Vec<u8>),
    Lambda(String, Box<Expr>),
    Lambdas(Vec<String>, Box<Expr>),
    App(Box<Expr>, Box<Expr>),
    AppN(Box<Expr>, Vec<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Let(String, Box<Expr>, Box<Expr>),
    Letrec(String, Box<Expr>, Box<Expr>),
    Ctor(String, Vec<Expr>),
    Match(Box<Expr>, Vec<MatchCase>),
    Prim(PrimOp, Vec<Expr>),
    Error,
    Extern(u16),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchCase {
    pub tag: String,
    pub bindings: Vec<String>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Define {
    pub name: String,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub defines: Vec<Define>,
}
