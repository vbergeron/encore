use crate::ir::asm::{Case, ContLam, Define, Expr, Fun, Module, Reg, Val, A1, A8, CONT, NULL, SELF};
use super::sexp::{self, arity, arity_at_least, form, int, list, sym, Node, NodeKind, Sexp};
use super::{parse_module, parse_prim, prim_sexp, TextError};

fn reg_name(r: Reg) -> String {
    match r {
        SELF => "self".into(),
        CONT => "cont".into(),
        NULL => "null".into(),
        A1..=A8 => format!("a{}", r - A1 + 1),
        _ => format!("r{r}"),
    }
}

fn reg(r: Reg) -> Sexp {
    sym(reg_name(r))
}

fn regs(rs: &[Reg]) -> impl Iterator<Item = Sexp> + '_ {
    rs.iter().map(|&r| reg(r))
}

fn parse_reg(node: &Node) -> Result<Reg, TextError> {
    let s = node.as_sym()?;
    let parsed = match s {
        "self" => Some(SELF),
        "cont" => Some(CONT),
        "null" => Some(NULL),
        _ => {
            let num = |p: &str| s.strip_prefix(p).filter(|d| !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()))
                .and_then(|d| d.parse::<u16>().ok());
            match (num("a"), num("r")) {
                (Some(n @ 1..=8), _) => Some(A1 + (n as u8 - 1)),
                (_, Some(n)) if n <= 0xFF => Some(n as u8),
                _ => None,
            }
        }
    };
    parsed.map_or_else(
        || node.err(format!("expected a register (self, cont, a1..a8, null, r0..r255), found `{s}`")),
        Ok,
    )
}

fn parse_regs(xs: &[Node]) -> Result<Vec<Reg>, TextError> {
    xs.iter().map(parse_reg).collect()
}

fn val_sexp(v: &Val) -> Sexp {
    match v {
        Val::Reg(r) => reg(*r),
        Val::Int(i) => int(*i),
        Val::Capture(i) => form("capture", [int(*i)]),
        Val::Global(g) => form("global", [int(*g)]),
        Val::ContLam(c) => form("cont-lam", [list(regs(&c.captures)), expr_sexp(&c.body)]),
        Val::Ctor(tag, fields) => form("ctor", std::iter::once(int(*tag)).chain(regs(fields))),
        Val::Field(r, idx) => form("field", [reg(*r), int(*idx)]),
        Val::Bytes(b) => form("bytes", [Sexp::Str(b.clone())]),
        Val::Prim(op, args) => form("prim", std::iter::once(prim_sexp(*op)).chain(regs(args))),
        Val::Extern(slot) => form("extern", [int(*slot)]),
    }
}

fn expr_sexp(e: &Expr) -> Sexp {
    match e {
        Expr::Let(r, v, body) => form("let", [reg(*r), val_sexp(v), expr_sexp(body)]),
        Expr::Letrec(r, fun, rest) => form(
            "letrec",
            [reg(*r), list(regs(&fun.captures)), expr_sexp(&fun.body), expr_sexp(rest)],
        ),
        Expr::Encore(f, args, k) => form("encore", [reg(*f), list(regs(args)), reg(*k)]),
        Expr::Match(scrut, base, cases) => form(
            "match",
            [reg(*scrut), int(*base)].into_iter().chain(cases.iter().map(|c| {
                form("case", [int(c.arity), reg(c.unpack_base), expr_sexp(&c.body)])
            })),
        ),
        Expr::Fin(r) => form("fin", [reg(*r)]),
    }
}

fn parse_val(node: &Node) -> Result<Val, TextError> {
    match &node.kind {
        NodeKind::Sym(_) => return Ok(Val::Reg(parse_reg(node)?)),
        NodeKind::Int(_) => return Ok(Val::Int(node.as_num("integer literal")?)),
        _ => {}
    }
    let Some((head, a)) = node.as_form() else {
        return node.err(format!("expected an asm value, found {}", node.describe()));
    };
    Ok(match head {
        "capture" => { arity(node, head, a, 1)?; Val::Capture(a[0].as_num("capture index")?) }
        "global" => { arity(node, head, a, 1)?; Val::Global(a[0].as_num("global index")?) }
        "cont-lam" => {
            arity(node, head, a, 2)?;
            Val::ContLam(ContLam { captures: parse_regs(a[0].as_list()?)?, body: Box::new(parse_expr(&a[1])?) })
        }
        "ctor" => { arity_at_least(node, head, a, 1)?; Val::Ctor(a[0].as_num("tag")?, parse_regs(&a[1..])?) }
        "field" => { arity(node, head, a, 2)?; Val::Field(parse_reg(&a[0])?, a[1].as_num("field index")?) }
        "bytes" => { arity(node, head, a, 1)?; Val::Bytes(a[0].as_str()?.to_vec()) }
        "prim" => { arity_at_least(node, head, a, 1)?; Val::Prim(parse_prim(&a[0])?, parse_regs(&a[1..])?) }
        "extern" => { arity(node, head, a, 1)?; Val::Extern(a[0].as_num("extern slot")?) }
        _ => return node.err(format!("unknown asm value form `{head}`")),
    })
}

fn parse_expr(node: &Node) -> Result<Expr, TextError> {
    let Some((head, a)) = node.as_form() else {
        return node.err(format!("expected an asm expression, found {}", node.describe()));
    };
    let boxed = |n: &Node| parse_expr(n).map(Box::new);
    Ok(match head {
        "let" => { arity(node, head, a, 3)?; Expr::Let(parse_reg(&a[0])?, parse_val(&a[1])?, boxed(&a[2])?) }
        "letrec" => {
            arity(node, head, a, 4)?;
            let fun = Fun { captures: parse_regs(a[1].as_list()?)?, body: boxed(&a[2])? };
            Expr::Letrec(parse_reg(&a[0])?, fun, boxed(&a[3])?)
        }
        "encore" => {
            arity(node, head, a, 3)?;
            Expr::Encore(parse_reg(&a[0])?, parse_regs(a[1].as_list()?)?, parse_reg(&a[2])?)
        }
        "match" => {
            arity_at_least(node, head, a, 2)?;
            let cases = a[2..].iter().map(|c| match c.as_form() {
                Some(("case", ca)) => {
                    arity(c, "case", ca, 3)?;
                    Ok(Case { arity: ca[0].as_num("arity")?, unpack_base: parse_reg(&ca[1])?, body: parse_expr(&ca[2])? })
                }
                _ => c.err(format!("expected `(case ...)`, found {}", c.describe())),
            }).collect::<Result<_, _>>()?;
            Expr::Match(parse_reg(&a[0])?, a[1].as_num("tag")?, cases)
        }
        "fin" => { arity(node, head, a, 1)?; Expr::Fin(parse_reg(&a[0])?) }
        _ => return node.err(format!("unknown asm expression form `{head}`")),
    })
}

impl Module {
    pub fn to_text(&self) -> String {
        let forms: Vec<Sexp> = self.defines.iter()
            .map(|d| form("define", [int(d.global), expr_sexp(&d.body)]))
            .collect();
        sexp::render_module("encore ir: asm", &forms)
    }

    pub fn from_text(src: &str) -> Result<Self, TextError> {
        let defines = parse_module(src, |node, a| {
            arity(node, "define", a, 2)?;
            Ok(Define { global: a[0].as_num("global index")?, body: parse_expr(&a[1])? })
        })?;
        Ok(Module { defines })
    }
}
