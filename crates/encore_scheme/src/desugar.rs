use std::collections::HashMap;

use encore_compiler::ir::ds;
use encore_compiler::ir::prim::{PrimOp, IntOp, BytesOp};

use crate::ir;
use crate::parser::Sexp;

// ---------------------------------------------------------------------------
// Phase 1: Sexp -> ir::Module
// ---------------------------------------------------------------------------

pub fn parse_program(sexps: &[Sexp]) -> Result<ir::Module, String> {
    let mut defs = Vec::new();
    let mut n_foreign: u16 = 0;
    for sexp in sexps {
        match sexp {
            Sexp::List(items) => {
                if let Some(Sexp::Atom(head)) = items.first() {
                    match head.as_str() {
                        "load" => continue,
                        "define" => defs.push(parse_define(items)?),
                        "define-foreign" => {
                            let def = parse_define_foreign(items, &mut n_foreign)?;
                            defs.push(def);
                        }
                        _ => return Err(format!("unexpected top-level form: {head}")),
                    }
                }
            }
            _ => return Err("unexpected top-level atom".to_string()),
        }
    }
    Ok(ir::Module { defines: defs })
}

fn parse_define(items: &[Sexp]) -> Result<ir::Define, String> {
    if items.len() != 3 {
        return Err("define expects 2 arguments".to_string());
    }
    let name = items[1]
        .as_atom()
        .ok_or("define name must be atom")?
        .to_string();
    let body = parse_expr(&items[2])?;
    Ok(ir::Define { name, body })
}

fn parse_define_foreign(items: &[Sexp], n_foreign: &mut u16) -> Result<ir::Define, String> {
    match items.len() {
        2 => {
            let name = items[1]
                .as_atom()
                .ok_or("define-foreign name must be an atom")?
                .to_string();
            let idx = *n_foreign;
            *n_foreign += 1;
            Ok(ir::Define {
                name,
                body: ir::Expr::Foreign(idx),
            })
        }
        3 => {
            let name = items[1]
                .as_atom()
                .ok_or("define-foreign name must be an atom")?
                .to_string();
            let params = items[2]
                .as_list()
                .ok_or("define-foreign params must be a list")?;
            if params.is_empty() {
                return Err("define-foreign params list must not be empty".to_string());
            }
            let param_names: Vec<String> = params
                .iter()
                .map(|p| {
                    p.as_atom()
                        .ok_or("define-foreign param must be an atom".to_string())
                        .map(|s| s.to_string())
                })
                .collect::<Result<_, _>>()?;
            let idx = *n_foreign;
            *n_foreign += 1;
            let pack_tag = format!("__ffi{idx}");
            let ctor_fields: Vec<ir::Expr> =
                param_names.iter().map(|p| ir::Expr::Var(p.clone())).collect();
            let body = ir::Expr::App(
                Box::new(ir::Expr::Foreign(idx)),
                Box::new(ir::Expr::Ctor(pack_tag, ctor_fields)),
            );
            let body = ir::Expr::Lambdas(param_names, Box::new(body));
            Ok(ir::Define { name, body })
        }
        _ => Err("define-foreign expects a name or a name and a params list".to_string()),
    }
}

fn parse_expr(sexp: &Sexp) -> Result<ir::Expr, String> {
    match sexp {
        Sexp::Atom(s) => {
            if let Ok(n) = s.parse::<i32>() {
                Ok(ir::Expr::Int(n))
            } else if s.starts_with('"') && s.ends_with('"') {
                let inner = &s[1..s.len() - 1];
                Ok(ir::Expr::Bytes(inner.as_bytes().to_vec()))
            } else {
                Ok(ir::Expr::Var(s.clone()))
            }
        }
        Sexp::List(items) if items.is_empty() => Err("empty application".to_string()),
        Sexp::List(items) => {
            let head = &items[0];
            if let Sexp::Atom(tag) = head {
                match tag.as_str() {
                    "lambda" => return parse_lambda(items),
                    "lambdas" => return parse_lambdas(items),
                    "@" => return parse_at(items),
                    "if" => return parse_if(items),
                    "let" => return parse_let(items),
                    "letrec" => return parse_letrec(items),
                    "match" => return parse_match(items),
                    "quote" => return parse_quote(&items[1]),
                    "quasiquote" => return parse_quasiquote(&items[1]),
                    "error" => return Ok(ir::Expr::Error),
                    "+" => return parse_binop(PrimOp::Int(IntOp::Add), items),
                    "-" => return parse_binop(PrimOp::Int(IntOp::Sub), items),
                    "*" => return parse_binop(PrimOp::Int(IntOp::Mul), items),
                    "=" => return parse_binop(PrimOp::Int(IntOp::Eq), items),
                    "<" => return parse_binop(PrimOp::Int(IntOp::Lt), items),
                    "int->byte"    => return parse_prim(PrimOp::Int(IntOp::Byte), 1, items),
                    "bytes-len"    => return parse_prim(PrimOp::Bytes(BytesOp::Len), 1, items),
                    "bytes-get"    => return parse_prim(PrimOp::Bytes(BytesOp::Get), 2, items),
                    "bytes-concat" => return parse_prim(PrimOp::Bytes(BytesOp::Concat), 2, items),
                    "bytes-slice"  => return parse_prim(PrimOp::Bytes(BytesOp::Slice), 3, items),
                    "bytes-eq"     => return parse_prim(PrimOp::Bytes(BytesOp::Eq), 2, items),
                    _ => {}
                }
            }
            parse_application(items)
        }
    }
}

fn parse_binop(op: PrimOp, items: &[Sexp]) -> Result<ir::Expr, String> {
    parse_prim(op, 2, items)
}

fn parse_prim(op: PrimOp, arity: usize, items: &[Sexp]) -> Result<ir::Expr, String> {
    if items.len() != arity + 1 {
        return Err(format!("{op:?} expects {arity} arguments"));
    }
    let args: Vec<ir::Expr> = items[1..]
        .iter()
        .map(parse_expr)
        .collect::<Result<_, _>>()?;
    Ok(ir::Expr::Prim(op, args))
}

fn parse_lambda(items: &[Sexp]) -> Result<ir::Expr, String> {
    if items.len() != 3 {
        return Err("lambda expects params and body".to_string());
    }
    let params = items[1]
        .as_list()
        .ok_or("lambda params must be a list")?;
    if params.len() != 1 {
        return Err(format!(
            "lambda must have exactly 1 param (use lambdas for multi), got {}",
            params.len()
        ));
    }
    let param = params[0]
        .as_atom()
        .ok_or("lambda param must be atom")?
        .to_string();
    let body = parse_expr(&items[2])?;
    Ok(ir::Expr::Lambda(param, Box::new(body)))
}

fn parse_lambdas(items: &[Sexp]) -> Result<ir::Expr, String> {
    if items.len() != 3 {
        return Err("lambdas expects params and body".to_string());
    }
    let params = items[1]
        .as_list()
        .ok_or("lambdas params must be a list")?;
    let param_names: Vec<String> = params
        .iter()
        .map(|p| {
            p.as_atom()
                .ok_or("lambdas param must be atom".to_string())
                .map(|s| s.to_string())
        })
        .collect::<Result<_, _>>()?;
    let body = parse_expr(&items[2])?;
    Ok(ir::Expr::Lambdas(param_names, Box::new(body)))
}

fn parse_at(items: &[Sexp]) -> Result<ir::Expr, String> {
    if items.len() < 2 {
        return Err("@ needs at least a function".to_string());
    }
    let func = parse_expr(&items[1])?;
    let args: Vec<ir::Expr> = items[2..]
        .iter()
        .map(parse_expr)
        .collect::<Result<_, _>>()?;
    if args.len() == 1 {
        Ok(ir::Expr::App(
            Box::new(func),
            Box::new(args.into_iter().next().unwrap()),
        ))
    } else if args.is_empty() {
        Ok(func)
    } else {
        Ok(ir::Expr::AppN(Box::new(func), args))
    }
}

fn parse_if(items: &[Sexp]) -> Result<ir::Expr, String> {
    if items.len() != 4 {
        return Err("if expects 3 arguments".to_string());
    }
    Ok(ir::Expr::If(
        Box::new(parse_expr(&items[1])?),
        Box::new(parse_expr(&items[2])?),
        Box::new(parse_expr(&items[3])?),
    ))
}

fn parse_let(items: &[Sexp]) -> Result<ir::Expr, String> {
    if items.len() != 3 {
        return Err("let expects bindings and body".to_string());
    }
    let bindings = items[1].as_list().ok_or("let bindings must be a list")?;
    let mut body = parse_expr(&items[2])?;
    for binding in bindings.iter().rev() {
        let pair = binding.as_list().ok_or("let binding must be a list")?;
        if pair.len() != 2 {
            return Err("let binding must have 2 elements".to_string());
        }
        let name = pair[0]
            .as_atom()
            .ok_or("let binding name must be atom")?
            .to_string();
        let val = parse_expr(&pair[1])?;
        body = ir::Expr::Let(name, Box::new(val), Box::new(body));
    }
    Ok(body)
}

fn parse_letrec(items: &[Sexp]) -> Result<ir::Expr, String> {
    if items.len() != 3 {
        return Err("letrec expects bindings and body".to_string());
    }
    let bindings = items[1].as_list().ok_or("letrec bindings must be a list")?;
    if bindings.len() != 1 {
        return Err("letrec supports only a single binding".to_string());
    }
    let pair = bindings[0]
        .as_list()
        .ok_or("letrec binding must be a list")?;
    if pair.len() != 2 {
        return Err("letrec binding must have 2 elements".to_string());
    }
    let name = pair[0]
        .as_atom()
        .ok_or("letrec binding name must be atom")?
        .to_string();
    let val = parse_expr(&pair[1])?;
    let body = parse_expr(&items[2])?;
    Ok(ir::Expr::Letrec(name, Box::new(val), Box::new(body)))
}

fn parse_match(items: &[Sexp]) -> Result<ir::Expr, String> {
    if items.len() < 3 {
        return Err("match needs scrutinee and at least one case".to_string());
    }
    let scrutinee = parse_expr(&items[1])?;
    let mut cases = Vec::new();
    for case_sexp in &items[2..] {
        let case_list = case_sexp.as_list().ok_or("match case must be a list")?;
        if case_list.len() != 2 {
            return Err("match case must have pattern and body".to_string());
        }
        let pattern = case_list[0]
            .as_list()
            .ok_or("match pattern must be a list")?;
        if pattern.is_empty() {
            return Err("match pattern must have a constructor".to_string());
        }
        let tag = pattern[0]
            .as_atom()
            .ok_or("match constructor must be atom")?
            .to_string();
        let bindings: Vec<String> = pattern[1..]
            .iter()
            .map(|s| {
                s.as_atom()
                    .ok_or("match binding must be atom".to_string())
                    .map(|a| a.to_string())
            })
            .collect::<Result<_, _>>()?;
        let body = parse_expr(&case_list[1])?;
        cases.push(ir::MatchCase {
            tag,
            bindings,
            body,
        });
    }
    Ok(ir::Expr::Match(Box::new(scrutinee), cases))
}

fn parse_quote(sexp: &Sexp) -> Result<ir::Expr, String> {
    match sexp {
        Sexp::Atom(s) => Ok(ir::Expr::Ctor(s.clone(), Vec::new())),
        Sexp::List(items) if items.is_empty() => {
            Ok(ir::Expr::Ctor("Nil".to_string(), Vec::new()))
        }
        Sexp::List(items) => {
            let tag = items[0]
                .as_atom()
                .ok_or("quoted list head must be atom")?
                .to_string();
            Ok(ir::Expr::Ctor(tag, Vec::new()))
        }
    }
}

fn is_ctor_tag(s: &str) -> bool {
    s.parse::<i32>().is_err()
}

fn parse_quasiquote(sexp: &Sexp) -> Result<ir::Expr, String> {
    match sexp {
        Sexp::Atom(s) => Ok(ir::Expr::Ctor(s.clone(), Vec::new())),
        Sexp::List(items) if items.is_empty() => {
            Ok(ir::Expr::Ctor("Nil".to_string(), Vec::new()))
        }
        Sexp::List(items) => match items[0].as_atom() {
            Some(tag) if is_ctor_tag(tag) => {
                let tag = tag.to_string();
                let mut fields = Vec::new();
                for item in &items[1..] {
                    match item {
                        Sexp::List(unq)
                            if unq.len() == 2 && unq[0].as_atom() == Some("unquote") =>
                        {
                            fields.push(parse_expr(&unq[1])?);
                        }
                        other => {
                            fields.push(parse_quasiquote(other)?);
                        }
                    }
                }
                Ok(ir::Expr::Ctor(tag, fields))
            }
            _ => {
                let parts: Vec<Sexp> = items
                    .iter()
                    .map(|item| match item {
                        Sexp::List(unq)
                            if unq.len() == 2 && unq[0].as_atom() == Some("unquote") =>
                        {
                            unq[1].clone()
                        }
                        other => other.clone(),
                    })
                    .collect();
                if parts.len() == 1 {
                    parse_expr(&parts[0])
                } else {
                    parse_application(&parts)
                }
            }
        },
    }
}

fn parse_application(items: &[Sexp]) -> Result<ir::Expr, String> {
    let func = parse_expr(&items[0])?;
    let args: Vec<ir::Expr> = items[1..]
        .iter()
        .map(parse_expr)
        .collect::<Result<_, _>>()?;
    if args.len() == 1 {
        Ok(ir::Expr::App(
            Box::new(func),
            Box::new(args.into_iter().next().unwrap()),
        ))
    } else if args.is_empty() {
        Ok(func)
    } else {
        Ok(ir::Expr::AppN(Box::new(func), args))
    }
}

// ---------------------------------------------------------------------------
// Phase 2: ir::Module -> ds::Module (lowering)
// ---------------------------------------------------------------------------

struct CtorInfo {
    tag: u8,
    #[allow(dead_code)]
    arity: u8,
}

struct Lowering {
    ctors: HashMap<String, CtorInfo>,
    next_tag: u8,
}

impl Lowering {
    fn new() -> Self {
        let mut ctors = HashMap::new();
        ctors.insert("False".into(), CtorInfo { tag: 0, arity: 0 });
        ctors.insert("True".into(), CtorInfo { tag: 1, arity: 0 });
        Self {
            ctors,
            next_tag: 2,
        }
    }

    fn resolve_tag(&mut self, name: &str, arity: u8) -> u8 {
        if let Some(info) = self.ctors.get(name) {
            return info.tag;
        }
        let tag = self.next_tag;
        self.next_tag += 1;
        self.ctors.insert(name.to_string(), CtorInfo { tag, arity });
        tag
    }

    fn ctor_names(&self) -> Vec<(u8, String)> {
        self.ctors
            .iter()
            .map(|(name, info)| (info.tag, name.clone()))
            .collect()
    }

    fn lower_module(&mut self, module: ir::Module) -> ds::Module {
        ds::Module {
            defines: module
                .defines
                .into_iter()
                .map(|d| ds::Define {
                    name: d.name,
                    body: self.lower_expr(d.body),
                })
                .collect(),
        }
    }

    fn lower_expr(&mut self, expr: ir::Expr) -> ds::Expr {
        match expr {
            ir::Expr::Var(name) => ds::Expr::Var(name),
            ir::Expr::Int(n) => ds::Expr::Int(n),
            ir::Expr::Bytes(data) => ds::Expr::Bytes(data),

            ir::Expr::Lambda(param, body) => {
                ds::Expr::Lam(param, Box::new(self.lower_expr(*body)))
            }

            ir::Expr::Lambdas(params, body) => {
                let body = self.lower_expr(*body);
                if params.len() == 1 {
                    ds::Expr::Lam(params.into_iter().next().unwrap(), Box::new(body))
                } else {
                    ds::Expr::LamN(params, Box::new(body))
                }
            }

            ir::Expr::App(f, arg) => ds::Expr::App(
                Box::new(self.lower_expr(*f)),
                Box::new(self.lower_expr(*arg)),
            ),

            ir::Expr::AppN(f, args) => {
                let f = self.lower_expr(*f);
                let args: Vec<ds::Expr> = args.into_iter().map(|a| self.lower_expr(a)).collect();
                if args.len() == 1 {
                    ds::Expr::App(Box::new(f), Box::new(args.into_iter().next().unwrap()))
                } else {
                    ds::Expr::AppN(Box::new(f), args)
                }
            }

            ir::Expr::If(cond, then_br, else_br) => {
                let cond = self.lower_expr(*cond);
                let then_br = self.lower_expr(*then_br);
                let else_br = self.lower_expr(*else_br);
                ds::Expr::Match(
                    Box::new(cond),
                    0,
                    vec![
                        ds::Case { binds: vec![], body: else_br },
                        ds::Case { binds: vec![], body: then_br },
                    ],
                )
            }

            ir::Expr::Let(name, val, body) => ds::Expr::Let(
                name,
                Box::new(self.lower_expr(*val)),
                Box::new(self.lower_expr(*body)),
            ),

            ir::Expr::Letrec(name, val, body) => {
                let val = self.lower_expr(*val);
                let body = self.lower_expr(*body);
                match val {
                    ds::Expr::Lam(param, fun_body) => {
                        ds::Expr::Letrec(name, param, fun_body, Box::new(body))
                    }
                    other => {
                        // Eta-expand: letrec name = val in body
                        // becomes:    letrec name __eta = (val __eta) in body
                        let eta = "__eta".to_string();
                        let fun_body = Box::new(ds::Expr::App(
                            Box::new(other),
                            Box::new(ds::Expr::Var(eta.clone())),
                        ));
                        ds::Expr::Letrec(name, eta, fun_body, Box::new(body))
                    }
                }
            }

            ir::Expr::Ctor(tag_name, fields) => {
                let arity = fields.len() as u8;
                let tag = self.resolve_tag(&tag_name, arity);
                ds::Expr::Ctor(tag, fields.into_iter().map(|f| self.lower_expr(f)).collect())
            }

            ir::Expr::Match(scrutinee, cases) => {
                let scrutinee = self.lower_expr(*scrutinee);
                let tagged_cases: Vec<(u8, ds::Case)> = cases
                    .into_iter()
                    .map(|c| {
                        let arity = c.bindings.len() as u8;
                        let tag = self.resolve_tag(&c.tag, arity);
                        let body = self.lower_expr(c.body);
                        (tag, ds::Case { binds: c.bindings, body })
                    })
                    .collect();

                let base_tag = tagged_cases.iter().map(|(t, _)| *t).min().unwrap_or(0);
                let max_tag = tagged_cases.iter().map(|(t, _)| *t).max().unwrap_or(0);
                let unreachable_case = ds::Case {
                    binds: vec![],
                    body: ds::Expr::Let(
                        "__err".to_string(),
                        Box::new(ds::Expr::Lam(
                            "x".to_string(),
                            Box::new(ds::Expr::Var("x".to_string())),
                        )),
                        Box::new(ds::Expr::App(
                            Box::new(ds::Expr::Var("__err".to_string())),
                            Box::new(ds::Expr::Var("__err".to_string())),
                        )),
                    ),
                };
                let mut sorted: Vec<ds::Case> = Vec::new();
                for tag in base_tag..=max_tag {
                    if let Some((_, case)) = tagged_cases.iter().find(|(t, _)| *t == tag) {
                        sorted.push(case.clone());
                    } else {
                        sorted.push(unreachable_case.clone());
                    }
                }

                ds::Expr::Match(Box::new(scrutinee), base_tag, sorted)
            }

            ir::Expr::Prim(op, args) => {
                ds::Expr::Prim(op, args.into_iter().map(|a| self.lower_expr(a)).collect())
            }

            ir::Expr::Error => {
                // Divergent term: let __err = (x -> x) in __err __err
                ds::Expr::Let(
                    "__err".to_string(),
                    Box::new(ds::Expr::Lam(
                        "x".to_string(),
                        Box::new(ds::Expr::Var("x".to_string())),
                    )),
                    Box::new(ds::Expr::App(
                        Box::new(ds::Expr::Var("__err".to_string())),
                        Box::new(ds::Expr::Var("__err".to_string())),
                    )),
                )
            }

            ir::Expr::Foreign(idx) => ds::Expr::Extern(idx),
        }
    }
}

pub fn lower_module(module: ir::Module) -> (ds::Module, Vec<(u8, String)>) {
    let mut lowering = Lowering::new();
    let ds_module = lowering.lower_module(module);
    (ds_module, lowering.ctor_names())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn parse_and_lower(src: &str) -> ds::Module {
        let sexps = parser::parse(src).unwrap();
        let scheme_module = parse_program(&sexps).unwrap();
        let (ds_module, _) = lower_module(scheme_module);
        ds_module
    }

    fn parse_scheme(src: &str) -> ir::Module {
        let sexps = parser::parse(src).unwrap();
        parse_program(&sexps).unwrap()
    }

    #[test]
    fn parse_negb() {
        let m = parse_scheme(
            r#"(define negb (lambda (b) (match b
                ((True) `(False))
                ((False) `(True)))))"#,
        );
        assert_eq!(m.defines.len(), 1);
        assert_eq!(m.defines[0].name, "negb");
        match &m.defines[0].body {
            ir::Expr::Lambda(param, body) => {
                assert_eq!(param, "b");
                match body.as_ref() {
                    ir::Expr::Match(_, cases) => {
                        assert_eq!(cases.len(), 2);
                        assert_eq!(cases[0].tag, "True");
                        assert_eq!(cases[1].tag, "False");
                    }
                    other => panic!("expected Match, got {other:?}"),
                }
            }
            other => panic!("expected Lambda, got {other:?}"),
        }
    }

    #[test]
    fn parse_lambdas_and_at() {
        let m = parse_scheme("(define f (lambdas (a b) (@ g a b)))");
        assert_eq!(m.defines.len(), 1);
        match &m.defines[0].body {
            ir::Expr::Lambdas(params, body) => {
                assert_eq!(params, &["a", "b"]);
                match body.as_ref() {
                    ir::Expr::AppN(_, args) => assert_eq!(args.len(), 2),
                    other => panic!("expected AppN, got {other:?}"),
                }
            }
            other => panic!("expected Lambdas, got {other:?}"),
        }
    }

    #[test]
    fn lower_preserves_lamn() {
        let m = parse_and_lower("(define f (lambdas (a b c) a))");
        match &m.defines[0].body {
            ds::Expr::LamN(params, _) => {
                assert_eq!(params, &["a", "b", "c"]);
            }
            _ => panic!("expected LamN"),
        }
    }

    #[test]
    fn lower_preserves_appn() {
        let m = parse_and_lower("(define r (@ f a b))");
        match &m.defines[0].body {
            ds::Expr::AppN(_, args) => {
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected AppN"),
        }
    }

    #[test]
    fn lower_if_to_match() {
        let m = parse_and_lower("(define r (if x 1 0))");
        match &m.defines[0].body {
            ds::Expr::Match(_, base_tag, cases) => {
                assert_eq!(*base_tag, 0);
                assert_eq!(cases.len(), 2);
                assert!(matches!(&cases[0].body, ds::Expr::Int(0)));
                assert!(matches!(&cases[1].body, ds::Expr::Int(1)));
            }
            _ => panic!("expected Match"),
        }
    }

    #[test]
    fn lower_letrec_lambda() {
        let m = parse_and_lower("(define r (letrec ((f (lambda (x) x))) (f 1)))");
        match &m.defines[0].body {
            ds::Expr::Letrec(fname, param, _, _) => {
                assert_eq!(fname, "f");
                assert_eq!(param, "x");
            }
            _ => panic!("expected Letrec"),
        }
    }

    #[test]
    fn lower_letrec_eta_expand() {
        let m = parse_and_lower("(define r (letrec ((f g)) (f 1)))");
        match &m.defines[0].body {
            ds::Expr::Letrec(fname, param, _, _) => {
                assert_eq!(fname, "f");
                assert_eq!(param, "__eta");
            }
            _ => panic!("expected Letrec"),
        }
    }

    #[test]
    fn load_skipped() {
        let m = parse_scheme(r#"(load "macros.scm") (define x (lambda (a) a))"#);
        assert_eq!(m.defines.len(), 1);
        assert_eq!(m.defines[0].name, "x");
    }

    #[test]
    fn ctor_tags_consistent() {
        let src = r#"
            (define f (lambda (x) (match x
                ((Nil) 0)
                ((Cons h t) h))))
            (define g (lambda (y) `(Cons ,y ,`(Nil))))
        "#;
        let sexps = parser::parse(src).unwrap();
        let scheme_module = parse_program(&sexps).unwrap();
        let (_, ctor_names) = lower_module(scheme_module);
        let nil_tag = ctor_names.iter().find(|(_, n)| n == "Nil").unwrap().0;
        let cons_tag = ctor_names.iter().find(|(_, n)| n == "Cons").unwrap().0;
        assert_ne!(nil_tag, cons_tag);
    }
}
