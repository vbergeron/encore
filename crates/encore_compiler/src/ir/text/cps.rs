use crate::ir::cps::{Case, Cont, Define, Expr, Fun, Module, Val};
use super::sexp::{self, arity, arity_at_least, form, int, sym, sym_list, Node, NodeKind, Sexp};
use super::{parse_module, parse_prim, prim_sexp, TextError};

fn syms(names: &[String]) -> impl Iterator<Item = Sexp> + '_ {
    names.iter().map(sym)
}

fn val_sexp(v: &Val) -> Sexp {
    match v {
        Val::Var(n) => sym(n),
        Val::Int(i) => int(*i),
        Val::Cont(c) => form("cont", [sym_list(&c.params), expr_sexp(&c.body)]),
        Val::NullCont => form("null-cont", []),
        Val::Ctor(tag, fields) => form("ctor", std::iter::once(int(*tag)).chain(syms(fields))),
        Val::Field(n, idx) => form("field", [sym(n), int(*idx)]),
        Val::Bytes(b) => form("bytes", [Sexp::Str(b.clone())]),
        Val::Prim(op, args) => form("prim", std::iter::once(prim_sexp(*op)).chain(syms(args))),
        Val::Extern(slot) => form("extern", [int(*slot)]),
    }
}

fn expr_sexp(e: &Expr) -> Sexp {
    match e {
        Expr::Let(x, v, body) => form("let", [sym(x), val_sexp(v), expr_sexp(body)]),
        Expr::Letrec(f, fun, rest) => form(
            "letrec",
            [sym(f), sym_list(&fun.args), sym(&fun.cont), expr_sexp(&fun.body), expr_sexp(rest)],
        ),
        Expr::Encore(f, args, k) => form("encore", [sym(f), sym_list(args), sym(k)]),
        Expr::Match(scrut, base, cases) => form(
            "match",
            [sym(scrut), int(*base)].into_iter().chain(
                cases.iter().map(|c| form("case", [sym_list(&c.binds), expr_sexp(&c.body)])),
            ),
        ),
        Expr::Fin(n) => form("fin", [sym(n)]),
    }
}

fn names(xs: &[Node]) -> Result<Vec<String>, TextError> {
    xs.iter().map(|n| n.as_sym().map(str::to_string)).collect()
}

fn parse_val(node: &Node) -> Result<Val, TextError> {
    match &node.kind {
        NodeKind::Sym(s) => return Ok(Val::Var(s.clone())),
        NodeKind::Int(_) => return Ok(Val::Int(node.as_num("integer literal")?)),
        _ => {}
    }
    let Some((head, a)) = node.as_form() else {
        return node.err(format!("expected a cps value, found {}", node.describe()));
    };
    Ok(match head {
        "cont" => {
            arity(node, head, a, 2)?;
            Val::Cont(Cont { params: a[0].sym_list()?, body: Box::new(parse_expr(&a[1])?) })
        }
        "null-cont" => { arity(node, head, a, 0)?; Val::NullCont }
        "ctor" => { arity_at_least(node, head, a, 1)?; Val::Ctor(a[0].as_num("tag")?, names(&a[1..])?) }
        "field" => { arity(node, head, a, 2)?; Val::Field(a[0].as_sym()?.into(), a[1].as_num("field index")?) }
        "bytes" => { arity(node, head, a, 1)?; Val::Bytes(a[0].as_str()?.to_vec()) }
        "prim" => { arity_at_least(node, head, a, 1)?; Val::Prim(parse_prim(&a[0])?, names(&a[1..])?) }
        "extern" => { arity(node, head, a, 1)?; Val::Extern(a[0].as_num("extern slot")?) }
        _ => return node.err(format!("unknown cps value form `{head}`")),
    })
}

fn parse_expr(node: &Node) -> Result<Expr, TextError> {
    let Some((head, a)) = node.as_form() else {
        return node.err(format!("expected a cps expression, found {}", node.describe()));
    };
    let boxed = |n: &Node| parse_expr(n).map(Box::new);
    Ok(match head {
        "let" => { arity(node, head, a, 3)?; Expr::Let(a[0].as_sym()?.into(), parse_val(&a[1])?, boxed(&a[2])?) }
        "letrec" => {
            arity(node, head, a, 5)?;
            let fun = Fun { args: a[1].sym_list()?, cont: a[2].as_sym()?.into(), body: boxed(&a[3])? };
            Expr::Letrec(a[0].as_sym()?.into(), fun, boxed(&a[4])?)
        }
        "encore" => {
            arity(node, head, a, 3)?;
            Expr::Encore(a[0].as_sym()?.into(), a[1].sym_list()?, a[2].as_sym()?.into())
        }
        "match" => {
            arity_at_least(node, head, a, 2)?;
            let cases = a[2..].iter().map(|c| match c.as_form() {
                Some(("case", ca)) => {
                    arity(c, "case", ca, 2)?;
                    Ok(Case { binds: ca[0].sym_list()?, body: parse_expr(&ca[1])? })
                }
                _ => c.err(format!("expected `(case ...)`, found {}", c.describe())),
            }).collect::<Result<_, _>>()?;
            Expr::Match(a[0].as_sym()?.into(), a[1].as_num("tag")?, cases)
        }
        "fin" => { arity(node, head, a, 1)?; Expr::Fin(a[0].as_sym()?.into()) }
        _ => return node.err(format!("unknown cps expression form `{head}`")),
    })
}

impl Module {
    pub fn to_text(&self) -> String {
        let forms: Vec<Sexp> = self.defines.iter()
            .map(|d| form("define", [sym(&d.name), expr_sexp(&d.body)]))
            .collect();
        sexp::render_module("encore ir: cps", &forms)
    }

    pub fn from_text(src: &str) -> Result<Self, TextError> {
        let defines = parse_module(src, |node, a| {
            arity(node, "define", a, 2)?;
            Ok(Define { name: a[0].as_sym()?.into(), body: parse_expr(&a[1])? })
        })?;
        Ok(Module { defines })
    }
}
