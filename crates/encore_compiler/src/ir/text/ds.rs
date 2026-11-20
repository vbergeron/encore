use crate::ir::ds::{Case, Define, Expr, Module};
use super::sexp::{self, arity, arity_at_least, form, int, sym, sym_list, Node, NodeKind, Sexp};
use super::{parse_module, parse_prim, prim_sexp, TextError};

fn expr_sexp(e: &Expr) -> Sexp {
    match e {
        Expr::Var(n) => sym(n),
        Expr::Int(i) => int(*i),
        Expr::Lambda(params, body) => form("lambda", [sym_list(params), expr_sexp(body)]),
        Expr::Apply(f, args) => form("app", std::iter::once(expr_sexp(f)).chain(args.iter().map(expr_sexp))),
        Expr::Let(x, bound, body) => form("let", [sym(x), expr_sexp(bound), expr_sexp(body)]),
        Expr::Letrec(f, x, fun_body, rest) => {
            form("letrec", [sym(f), sym(x), expr_sexp(fun_body), expr_sexp(rest)])
        }
        Expr::Ctor(tag, fields) => form("ctor", std::iter::once(int(*tag)).chain(fields.iter().map(expr_sexp))),
        Expr::Field(e, idx) => form("field", [expr_sexp(e), int(*idx)]),
        Expr::Match(scrut, base, cases) => form(
            "match",
            [expr_sexp(scrut), int(*base)].into_iter().chain(
                cases.iter().map(|c| form("case", [sym_list(&c.binds), expr_sexp(&c.body)])),
            ),
        ),
        Expr::Bytes(b) => form("bytes", [Sexp::Str(b.clone())]),
        Expr::Prim(op, args) => form("prim", std::iter::once(prim_sexp(*op)).chain(args.iter().map(expr_sexp))),
        Expr::Extern(slot) => form("extern", [int(*slot)]),
    }
}

fn parse_expr(node: &Node) -> Result<Expr, TextError> {
    match &node.kind {
        NodeKind::Sym(s) => return Ok(Expr::Var(s.clone())),
        NodeKind::Int(_) => return Ok(Expr::Int(node.as_num("integer literal")?)),
        _ => {}
    }
    let Some((head, a)) = node.as_form() else {
        return node.err(format!("expected a ds expression, found {}", node.describe()));
    };
    let exprs = |xs: &[Node]| xs.iter().map(parse_expr).collect::<Result<Vec<_>, _>>();
    let boxed = |n: &Node| parse_expr(n).map(Box::new);
    Ok(match head {
        "lambda" => { arity(node, head, a, 2)?; Expr::Lambda(a[0].sym_list()?, boxed(&a[1])?) }
        "app" => { arity_at_least(node, head, a, 1)?; Expr::Apply(boxed(&a[0])?, exprs(&a[1..])?) }
        "let" => { arity(node, head, a, 3)?; Expr::Let(a[0].as_sym()?.into(), boxed(&a[1])?, boxed(&a[2])?) }
        "letrec" => {
            arity(node, head, a, 4)?;
            Expr::Letrec(a[0].as_sym()?.into(), a[1].as_sym()?.into(), boxed(&a[2])?, boxed(&a[3])?)
        }
        "ctor" => { arity_at_least(node, head, a, 1)?; Expr::Ctor(a[0].as_num("tag")?, exprs(&a[1..])?) }
        "field" => { arity(node, head, a, 2)?; Expr::Field(boxed(&a[0])?, a[1].as_num("field index")?) }
        "match" => {
            arity_at_least(node, head, a, 2)?;
            let cases = a[2..].iter().map(|c| match c.as_form() {
                Some(("case", ca)) => {
                    arity(c, "case", ca, 2)?;
                    Ok(Case { binds: ca[0].sym_list()?, body: parse_expr(&ca[1])? })
                }
                _ => c.err(format!("expected `(case ...)`, found {}", c.describe())),
            }).collect::<Result<_, _>>()?;
            Expr::Match(boxed(&a[0])?, a[1].as_num("tag")?, cases)
        }
        "bytes" => { arity(node, head, a, 1)?; Expr::Bytes(a[0].as_str()?.to_vec()) }
        "prim" => { arity_at_least(node, head, a, 1)?; Expr::Prim(parse_prim(&a[0])?, exprs(&a[1..])?) }
        "extern" => { arity(node, head, a, 1)?; Expr::Extern(a[0].as_num("extern slot")?) }
        _ => return node.err(format!("unknown ds form `{head}`")),
    })
}

impl Module {
    pub fn to_text(&self) -> String {
        let forms: Vec<Sexp> = self.defines.iter()
            .map(|d| form("define", [sym(&d.name), expr_sexp(&d.body)]))
            .collect();
        sexp::render_module("encore ir: ds", &forms)
    }

    pub fn from_text(src: &str) -> Result<Self, TextError> {
        let defines = parse_module(src, |node, a| {
            arity(node, "define", a, 2)?;
            Ok(Define { name: a[0].as_sym()?.into(), body: parse_expr(&a[1])? })
        })?;
        Ok(Module { defines })
    }
}
