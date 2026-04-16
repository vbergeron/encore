use std::fmt;
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
    pub params: Vec<Name>,
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
    Bytes(Vec<u8>),
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

// ── Pretty-printing ─────────────────────────────────────────────────────────

struct Indent(usize);

impl Indent {
    fn next(&self) -> Indent { Indent(self.0 + 1) }
}

impl fmt::Display for Indent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..self.0 { write!(f, "  ")?; }
        Ok(())
    }
}

impl fmt::Display for PrimOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrimOp::Int(op) => match op {
                super::prim::IntOp::Add => write!(f, "int.add"),
                super::prim::IntOp::Sub => write!(f, "int.sub"),
                super::prim::IntOp::Mul => write!(f, "int.mul"),
                super::prim::IntOp::Eq  => write!(f, "int.eq"),
                super::prim::IntOp::Lt  => write!(f, "int.lt"),
                super::prim::IntOp::Byte => write!(f, "int.byte"),
            },
            PrimOp::Bytes(op) => match op {
                super::prim::BytesOp::Len    => write!(f, "bytes.len"),
                super::prim::BytesOp::Get    => write!(f, "bytes.get"),
                super::prim::BytesOp::Concat => write!(f, "bytes.concat"),
                super::prim::BytesOp::Slice  => write!(f, "bytes.slice"),
                super::prim::BytesOp::Eq     => write!(f, "bytes.eq"),
            },
        }
    }
}

fn fmt_val(val: &Val, ind: &Indent, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match val {
        Val::Var(n) => write!(f, "{n}"),
        Val::Int(i) => write!(f, "{i}"),
        Val::NullCont => write!(f, "null_cont"),
        Val::Extern(slot) => write!(f, "extern({slot})"),
        Val::Field(n, idx) => write!(f, "field({n}, {idx})"),
        Val::Ctor(tag, fields) => {
            write!(f, "ctor({tag}")?;
            for fld in fields { write!(f, ", {fld}")?; }
            write!(f, ")")
        }
        Val::Bytes(bs) => write!(f, "bytes({bs:?})"),
        Val::Prim(op, args) => {
            write!(f, "{op}({})", args.join(", "))
        }
        Val::Cont(cont) => {
            write!(f, "cont({}) =>\n", cont.params.join(", "))?;
            fmt_expr(&cont.body, &ind.next(), f)
        }
    }
}

fn fmt_expr(expr: &Expr, ind: &Indent, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match expr {
        Expr::Fin(n) => write!(f, "{ind}fin {n}"),
        Expr::Encore(func, args, k) => {
            write!(f, "{ind}encore {func}({}) -> {k}", args.join(", "))
        }
        Expr::Let(name, val, body) => {
            write!(f, "{ind}let {name} = ")?;
            fmt_val(val, ind, f)?;
            write!(f, "\n")?;
            fmt_expr(body, ind, f)
        }
        Expr::Letrec(name, fun, body) => {
            write!(f, "{ind}letrec {name}({}) -> {} =\n",
                fun.args.join(", "), fun.cont)?;
            fmt_expr(&fun.body, &ind.next(), f)?;
            write!(f, "\n")?;
            fmt_expr(body, ind, f)
        }
        Expr::Match(scrut, base_tag, cases) => {
            write!(f, "{ind}match {scrut} (base={base_tag})")?;
            for (i, case) in cases.iter().enumerate() {
                let tag = *base_tag as usize + i;
                if case.binds.is_empty() {
                    write!(f, "\n{ind}| {tag} ->\n")?;
                } else {
                    write!(f, "\n{ind}| {tag}({}) ->\n", case.binds.join(", "))?;
                }
                fmt_expr(&case.body, &ind.next(), f)?;
            }
            Ok(())
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_expr(self, &Indent(0), f)
    }
}

impl fmt::Display for Define {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "define {} =\n{}", self.name, self.body)
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, d) in self.defines.iter().enumerate() {
            if i > 0 { write!(f, "\n\n")?; }
            write!(f, "{d}")?;
        }
        Ok(())
    }
}
