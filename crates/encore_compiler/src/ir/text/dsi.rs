use crate::ir::dsi::{Case, Define, Expr, Module};
use super::sexp::{self, arity, arity_at_least, form, int, sym, Node, NodeKind, Sexp};
use super::{parse_module, parse_prim, prim_sexp, TextError};

fn usize_sexp(n: usize) -> Sexp {
    int(n as i64)
}

fn expr_sexp(e: &Expr) -> Sexp {
    match e {
        Expr::Var(idx) => form("var", [usize_sexp(*idx)]),
        Expr::Int(i) => int(*i),
        Expr::Lambda(n, body) => form("lambda", [usize_sexp(*n), expr_sexp(body)]),
        Expr::Apply(f, args) => form("app", std::iter::once(expr_sexp(f)).chain(args.iter().map(expr_sexp))),
        Expr::Let(bound, body) => form("let", [expr_sexp(bound), expr_sexp(body)]),
        Expr::Letrec(fun_body, rest) => form("letrec", [expr_sexp(fun_body), expr_sexp(rest)]),
        Expr::Ctor(tag, fields) => form("ctor", std::iter::once(int(*tag)).chain(fields.iter().map(expr_sexp))),
        Expr::Field(e, idx) => form("field", [expr_sexp(e), int(*idx)]),
        Expr::Match(scrut, base, cases) => form(
            "match",
            [expr_sexp(scrut), int(*base)].into_iter().chain(
                cases.iter().map(|c| form("case", [usize_sexp(c.arity), expr_sexp(&c.body)])),
            ),
        ),
        Expr::Bytes(b) => form("bytes", [Sexp::Str(b.clone())]),
        Expr::Prim(op, args) => form("prim", std::iter::once(prim_sexp(*op)).chain(args.iter().map(expr_sexp))),
        Expr::Extern(slot) => form("extern", [int(*slot)]),
    }
}

fn parse_expr(node: &Node) -> Result<Expr, TextError> {
    if let NodeKind::Int(_) = node.kind {
        return Ok(Expr::Int(node.as_num("integer literal")?));
    }
    let Some((head, a)) = node.as_form() else {
        return node.err(format!("expected a dsi expression, found {}", node.describe()));
    };
    let exprs = |xs: &[Node]| xs.iter().map(parse_expr).collect::<Result<Vec<_>, _>>();
    let boxed = |n: &Node| parse_expr(n).map(Box::new);
    Ok(match head {
        "var" => { arity(node, head, a, 1)?; Expr::Var(a[0].as_num("de Bruijn index")?) }
        "lambda" => { arity(node, head, a, 2)?; Expr::Lambda(a[0].as_num("arity")?, boxed(&a[1])?) }
        "app" => { arity_at_least(node, head, a, 1)?; Expr::Apply(boxed(&a[0])?, exprs(&a[1..])?) }
        "let" => { arity(node, head, a, 2)?; Expr::Let(boxed(&a[0])?, boxed(&a[1])?) }
        "letrec" => { arity(node, head, a, 2)?; Expr::Letrec(boxed(&a[0])?, boxed(&a[1])?) }
        "ctor" => { arity_at_least(node, head, a, 1)?; Expr::Ctor(a[0].as_num("tag")?, exprs(&a[1..])?) }
        "field" => { arity(node, head, a, 2)?; Expr::Field(boxed(&a[0])?, a[1].as_num("field index")?) }
        "match" => {
            arity_at_least(node, head, a, 2)?;
            let cases = a[2..].iter().map(|c| match c.as_form() {
                Some(("case", ca)) => {
                    arity(c, "case", ca, 2)?;
                    Ok(Case { arity: ca[0].as_num("arity")?, body: parse_expr(&ca[1])? })
                }
                _ => c.err(format!("expected `(case ...)`, found {}", c.describe())),
            }).collect::<Result<_, _>>()?;
            Expr::Match(boxed(&a[0])?, a[1].as_num("tag")?, cases)
        }
        "bytes" => { arity(node, head, a, 1)?; Expr::Bytes(a[0].as_str()?.to_vec()) }
        "prim" => { arity_at_least(node, head, a, 1)?; Expr::Prim(parse_prim(&a[0])?, exprs(&a[1..])?) }
        "extern" => { arity(node, head, a, 1)?; Expr::Extern(a[0].as_num("extern slot")?) }
        _ => return node.err(format!("unknown dsi form `{head}`")),
    })
}

impl Module {
    pub fn to_text(&self) -> String {
        let forms: Vec<Sexp> = self.defines.iter()
            .map(|d| form("define", [sym(&d.name), expr_sexp(&d.body)]))
            .collect();
        sexp::render_module("encore ir: dsi", &forms)
    }

    pub fn from_text(src: &str) -> Result<Self, TextError> {
        let defines = parse_module(src, |node, a| {
            arity(node, "define", a, 2)?;
            Ok(Define { name: a[0].as_sym()?.into(), body: parse_expr(&a[1])? })
        })?;
        Ok(Module { defines })
    }
}
