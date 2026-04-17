use std::collections::BTreeMap;

use encore_compiler::ir::ds;
use encore_compiler::ir::prim::{PrimOp, IntOp, BytesOp};
use encore_vm::builtins::{
    ARITY_CONS, ARITY_FALSE, ARITY_NIL, ARITY_TRUE, FIRST_USER_TAG, TAG_CONS, TAG_FALSE, TAG_NIL,
    TAG_TRUE,
};

use crate::ir;
use crate::parser::Sexp;

// ---------------------------------------------------------------------------
// Phase 1: Sexp -> ir::Module
// ---------------------------------------------------------------------------

pub fn parse_program(sexps: &[Sexp]) -> Result<ir::Module, String> {
    let mut defs = Vec::new();
    let mut n_extern: u16 = 0;
    for sexp in sexps {
        match sexp {
            Sexp::List(items) => {
                if let Some(Sexp::Atom(head)) = items.first() {
                    match head.as_str() {
                        "load" => continue,
                        "define" => {
                            if let Some(result) = parse_extern_define(items) {
                                defs.push(result?);
                            } else {
                                defs.push(parse_define(items)?);
                            }
                        }
                        "define-extern" => {
                            let def = parse_define_extern(items, &mut n_extern)?;
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

/// Recognize `(define name (extern (slot N) arg1 arg2 ...))`.
fn parse_extern_define(items: &[Sexp]) -> Option<Result<ir::Define, String>> {
    if items.len() != 3 {
        return None;
    }
    let body_items = items[2].as_list()?;
    if body_items.first()?.as_atom()? != "extern" {
        return None;
    }
    Some(parse_extern_define_inner(items, body_items))
}

fn parse_extern_define_inner(items: &[Sexp], body_items: &[Sexp]) -> Result<ir::Define, String> {
    let name = items[1]
        .as_atom()
        .ok_or("define name must be an atom")?
        .to_string();
    if body_items.len() < 2 {
        return Err("(extern (slot N) ...) expected".to_string());
    }
    let slot_form = body_items[1]
        .as_list()
        .ok_or("extern slot must be (slot N)")?;
    if slot_form.len() != 2 || slot_form[0].as_atom() != Some("slot") {
        return Err("extern slot must be (slot N)".to_string());
    }
    let slot_str = slot_form[1]
        .as_atom()
        .ok_or("slot index must be a number")?;
    let idx: u16 = slot_str
        .parse()
        .map_err(|_| format!("slot index must be a u16, got {slot_str}"))?;
    if body_items.len() == 2 {
        return Ok(ir::Define {
            name,
            body: ir::Expr::Extern(idx),
        });
    }
    let param_names: Vec<String> = body_items[2..]
        .iter()
        .map(|p| {
            p.as_atom()
                .ok_or("extern param must be an atom".to_string())
                .map(|s| s.to_string())
        })
        .collect::<Result<_, _>>()?;
    let pack_tag = format!("__ffi{idx}");
    let ctor_fields: Vec<ir::Expr> = param_names.iter().map(|p| ir::Expr::Var(p.clone())).collect();
    let body = ir::Expr::App(
        Box::new(ir::Expr::Extern(idx)),
        Box::new(ir::Expr::Ctor(pack_tag, ctor_fields)),
    );
    let body = ir::Expr::Lambdas(param_names, Box::new(body));
    Ok(ir::Define { name, body })
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

fn parse_define_extern(items: &[Sexp], n_extern: &mut u16) -> Result<ir::Define, String> {
    match items.len() {
        2 => {
            let name = items[1]
                .as_atom()
                .ok_or("define-extern name must be an atom")?
                .to_string();
            let idx = *n_extern;
            *n_extern += 1;
            Ok(ir::Define {
                name,
                body: ir::Expr::Extern(idx),
            })
        }
        3 => {
            let name = items[1]
                .as_atom()
                .ok_or("define-extern name must be an atom")?
                .to_string();
            let params = items[2]
                .as_list()
                .ok_or("define-extern params must be a list")?;
            if params.is_empty() {
                return Err("define-extern params list must not be empty".to_string());
            }
            let param_names: Vec<String> = params
                .iter()
                .map(|p| {
                    p.as_atom()
                        .ok_or("define-extern param must be an atom".to_string())
                        .map(|s| s.to_string())
                })
                .collect::<Result<_, _>>()?;
            let idx = *n_extern;
            *n_extern += 1;
            let pack_tag = format!("__ffi{idx}");
            let ctor_fields: Vec<ir::Expr> =
                param_names.iter().map(|p| ir::Expr::Var(p.clone())).collect();
            let body = ir::Expr::App(
                Box::new(ir::Expr::Extern(idx)),
                Box::new(ir::Expr::Ctor(pack_tag, ctor_fields)),
            );
            let body = ir::Expr::Lambdas(param_names, Box::new(body));
            Ok(ir::Define { name, body })
        }
        _ => Err("define-extern expects a name or a name and a params list".to_string()),
    }
}

fn parse_int(s: &str) -> Option<i32> {
    if let Ok(n) = s.parse::<i32>() {
        return Some(n);
    }
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
    i32::from_str_radix(hex, 16).ok()
}

fn parse_expr(sexp: &Sexp) -> Result<ir::Expr, String> {
    match sexp {
        Sexp::Atom(s) => {
            if let Some(n) = parse_int(s) {
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
    parse_int(s).is_none()
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
    ctors: BTreeMap<String, CtorInfo>,
    next_tag: u8,
}

impl Lowering {
    fn new() -> Self {
        let mut ctors = BTreeMap::new();
        ctors.insert("False".into(), CtorInfo { tag: TAG_FALSE, arity: ARITY_FALSE });
        ctors.insert("True".into(), CtorInfo { tag: TAG_TRUE, arity: ARITY_TRUE });
        ctors.insert("Nil".into(), CtorInfo { tag: TAG_NIL, arity: ARITY_NIL });
        ctors.insert("Cons".into(), CtorInfo { tag: TAG_CONS, arity: ARITY_CONS });
        Self {
            ctors,
            next_tag: FIRST_USER_TAG,
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
                ds::Expr::Lambda(vec![param], Box::new(self.lower_expr(*body)))
            }

            ir::Expr::Lambdas(params, body) => {
                let body = self.lower_expr(*body);
                params.into_iter().rev().fold(body, |b, p| {
                    ds::Expr::Lambda(vec![p], Box::new(b))
                })
            }

            ir::Expr::App(f, arg) => ds::Expr::Apply(
                Box::new(self.lower_expr(*f)),
                vec![self.lower_expr(*arg)],
            ),

            ir::Expr::AppN(f, args) => {
                let f = self.lower_expr(*f);
                let args: Vec<ds::Expr> = args.into_iter().map(|a| self.lower_expr(a)).collect();
                ds::Expr::Apply(Box::new(f), args)
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
                    ds::Expr::Lambda(mut params, fun_body) if params.len() == 1 => {
                        let param = params.pop().unwrap();
                        ds::Expr::Letrec(name, param, fun_body, Box::new(body))
                    }
                    other => {
                        let eta = "__eta".to_string();
                        let fun_body = Box::new(ds::Expr::Apply(
                            Box::new(other),
                            vec![ds::Expr::Var(eta.clone())],
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
                let pre_tagged: Vec<(u8, ir::MatchCase)> = cases
                    .into_iter()
                    .map(|c| {
                        let arity = c.bindings.len() as u8;
                        let tag = self.resolve_tag(&c.tag, arity);
                        (tag, c)
                    })
                    .collect();
                let tagged_cases: Vec<(u8, ds::Case)> = pre_tagged
                    .into_iter()
                    .map(|(tag, c)| {
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
                        Box::new(ds::Expr::Lambda(
                            vec!["x".to_string()],
                            Box::new(ds::Expr::Var("x".to_string())),
                        )),
                        Box::new(ds::Expr::Apply(
                            Box::new(ds::Expr::Var("__err".to_string())),
                            vec![ds::Expr::Var("__err".to_string())],
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
                ds::Expr::Let(
                    "__err".to_string(),
                    Box::new(ds::Expr::Lambda(
                        vec!["x".to_string()],
                        Box::new(ds::Expr::Var("x".to_string())),
                    )),
                    Box::new(ds::Expr::Apply(
                        Box::new(ds::Expr::Var("__err".to_string())),
                        vec![ds::Expr::Var("__err".to_string())],
                    )),
                )
            }

            ir::Expr::Extern(idx) => ds::Expr::Extern(idx),
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
    fn lower_curries_lambdas() {
        let m = parse_and_lower("(define f (lambdas (a b c) a))");
        match &m.defines[0].body {
            ds::Expr::Lambda(p1, b1) => {
                assert_eq!(p1, &["a"]);
                match b1.as_ref() {
                    ds::Expr::Lambda(p2, b2) => {
                        assert_eq!(p2, &["b"]);
                        match b2.as_ref() {
                            ds::Expr::Lambda(p3, _) => assert_eq!(p3, &["c"]),
                            _ => panic!("expected inner Lambda"),
                        }
                    }
                    _ => panic!("expected middle Lambda"),
                }
            }
            _ => panic!("expected Lambda"),
        }
    }

    #[test]
    fn lower_preserves_apply() {
        let m = parse_and_lower("(define r (@ f a b))");
        match &m.defines[0].body {
            ds::Expr::Apply(_, args) => {
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected Apply"),
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
    fn define_extern_inline_no_params() {
        let m = parse_and_lower("(define f (extern (slot 0)))");
        assert_eq!(m.defines.len(), 1);
        assert_eq!(m.defines[0].name, "f");
        assert!(matches!(&m.defines[0].body, ds::Expr::Extern(0)));
    }

    #[test]
    fn define_extern_inline_with_params() {
        let m = parse_and_lower("(define sign (extern (slot 3) bytes))");
        assert_eq!(m.defines.len(), 1);
        assert_eq!(m.defines[0].name, "sign");
        match &m.defines[0].body {
            ds::Expr::Lambda(params, body) => {
                assert_eq!(params, &["bytes"]);
                match body.as_ref() {
                    ds::Expr::Apply(f, _) => {
                        assert!(matches!(f.as_ref(), ds::Expr::Extern(3)));
                    }
                    _ => panic!("expected Apply"),
                }
            }
            _ => panic!("expected Lambda"),
        }
    }

    #[test]
    fn define_extern_inline_multi_params() {
        let m = parse_and_lower("(define f (extern (slot 1) a b c))");
        assert_eq!(m.defines.len(), 1);
        assert_eq!(m.defines[0].name, "f");
        match &m.defines[0].body {
            ds::Expr::Lambda(p1, inner) => {
                assert_eq!(p1, &["a"]);
                match inner.as_ref() {
                    ds::Expr::Lambda(p2, inner2) => {
                        assert_eq!(p2, &["b"]);
                        match inner2.as_ref() {
                            ds::Expr::Lambda(p3, _) => assert_eq!(p3, &["c"]),
                            _ => panic!("expected inner Lambda"),
                        }
                    }
                    _ => panic!("expected middle Lambda"),
                }
            }
            _ => panic!("expected Lambda"),
        }
    }

    #[test]
    fn define_extern_inline_explicit_slots() {
        let m = parse_and_lower(
            "(define a (extern (slot 5))) (define b (extern (slot 2) x)) (define-extern c)",
        );
        assert_eq!(m.defines.len(), 3);
        assert!(matches!(&m.defines[0].body, ds::Expr::Extern(5)));
        match &m.defines[1].body {
            ds::Expr::Lambda(_, body) => match body.as_ref() {
                ds::Expr::Apply(f, _) => assert!(matches!(f.as_ref(), ds::Expr::Extern(2))),
                _ => panic!("expected Apply"),
            },
            _ => panic!("expected Lambda"),
        }
        assert!(matches!(&m.defines[2].body, ds::Expr::Extern(0)));
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
