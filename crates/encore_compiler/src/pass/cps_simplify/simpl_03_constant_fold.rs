// Evaluate primitive operations on known constants, resolve field
// accesses on known constructors, and eliminate matches on values
// whose constructor tag is statically known.
//
//   let x = 3 in let y = 4 in builtin add x y     ──►   7
//
//   let r = eq(3, 3) in match r                    ──►   <true branch>
//     | False -> ...
//     | True  -> e
//
//   let p = Ctor(0, [a, b]) in field 0 of p        ──►   a
//
//   let s = bytes [68 69] in bytes_len s            ──►   2
//

use std::collections::HashMap;

use crate::ir::cps::{self, Expr, Fun, Cont, Val, Tag};
use crate::ir::prim::{PrimOp, IntOp, BytesOp};
use crate::pass::cps_subst::subst_expr;

#[derive(Clone)]
enum Known {
    Int(i32),
    Bytes(Vec<u8>),
    Ctor(Tag, Vec<String>),
}

type Env = HashMap<String, Known>;

pub fn constant_fold(expr: Expr) -> Expr {
    constant_fold_env(expr, &mut Env::new())
}

fn record(env: &mut Env, name: &str, val: &Val) {
    match val {
        Val::Int(n) => { env.insert(name.to_string(), Known::Int(*n)); }
        Val::Bytes(data) => { env.insert(name.to_string(), Known::Bytes(data.clone())); }
        Val::Ctor(tag, fields) => { env.insert(name.to_string(), Known::Ctor(*tag, fields.clone())); }
        _ => {}
    }
}

fn constant_fold_env(expr: Expr, env: &mut Env) -> Expr {
    match expr {
        Expr::Let(name, Val::Int(n), body) => {
            env.insert(name.clone(), Known::Int(n));
            let body = constant_fold_env(*body, env);
            Expr::Let(name, Val::Int(n), Box::new(body))
        }
        Expr::Let(name, Val::Bytes(data), body) => {
            env.insert(name.clone(), Known::Bytes(data.clone()));
            let body = constant_fold_env(*body, env);
            Expr::Let(name, Val::Bytes(data), Box::new(body))
        }
        Expr::Let(name, Val::Ctor(tag, fields), body) => {
            env.insert(name.clone(), Known::Ctor(tag, fields.clone()));
            let body = constant_fold_env(*body, env);
            Expr::Let(name, Val::Ctor(tag, fields), Box::new(body))
        }
        Expr::Let(name, Val::Field(x, idx), body) => {
            if let Some(Known::Ctor(_, fields)) = env.get(&x).cloned() {
                if (idx as usize) < fields.len() {
                    let target = fields[idx as usize].clone();
                    let body = constant_fold_env(*body, env);
                    return Expr::Let(name, Val::Var(target), Box::new(body));
                }
            }
            let body = constant_fold_env(*body, env);
            Expr::Let(name, Val::Field(x, idx), Box::new(body))
        }
        Expr::Let(name, Val::Prim(op, args), body) => {
            if let Some(val) = try_fold_prim(op, &args, env) {
                record(env, &name, &val);
                let body = constant_fold_env(*body, env);
                Expr::Let(name, val, Box::new(body))
            } else {
                let body = constant_fold_env(*body, env);
                Expr::Let(name, Val::Prim(op, args), Box::new(body))
            }
        }
        Expr::Let(name, val, body) => {
            let val = constant_fold_val(val, env);
            let body = constant_fold_env(*body, env);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, fun, body) => {
            let fun = constant_fold_fun(fun, env);
            let body = constant_fold_env(*body, env);
            Expr::Letrec(name, fun, Box::new(body))
        }
        Expr::Match(name, base, cases) => {
            if let Some(Known::Ctor(tag, fields)) = env.get(&name).cloned() {
                let branch = tag.wrapping_sub(base) as usize;
                if branch < cases.len() {
                    let case = cases.into_iter().nth(branch).unwrap();
                    let mut body = case.body;
                    for (bind, field) in case.binds.iter().zip(fields.iter()) {
                        subst_expr(bind, field, &mut body);
                    }
                    return constant_fold_env(body, env);
                }
            }
            let cases = cases
                .into_iter()
                .map(|c| cps::Case {
                    binds: c.binds,
                    body: constant_fold_env(c.body, &mut env.clone()),
                })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn constant_fold_val(val: Val, env: &mut Env) -> Val {
    match val {
        Val::Cont(cont) => Val::Cont(constant_fold_cont(cont, env)),
        other => other,
    }
}

fn constant_fold_fun(fun: Fun, env: &mut Env) -> Fun {
    Fun {
        args: fun.args,
        cont: fun.cont,
        body: Box::new(constant_fold_env(*fun.body, &mut env.clone())),
    }
}

fn constant_fold_cont(cont: Cont, env: &mut Env) -> Cont {
    Cont {
        params: cont.params,
        body: Box::new(constant_fold_env(*cont.body, &mut env.clone())),
    }
}

fn try_fold_prim(op: PrimOp, args: &[String], env: &Env) -> Option<Val> {
    match op {
        PrimOp::Int(IntOp::Byte) => {
            match env.get(&args[0]) {
                Some(Known::Int(n)) if *n >= 0 && *n <= 255 => {
                    Some(Val::Bytes(vec![*n as u8]))
                }
                _ => None,
            }
        }
        PrimOp::Int(iop) => {
            match (env.get(&args[0]), env.get(&args[1])) {
                (Some(Known::Int(a)), Some(Known::Int(b))) => {
                    Some(eval_int_prim(iop, *a, *b))
                }
                _ => None,
            }
        }
        PrimOp::Bytes(bop) => try_fold_bytes_prim(bop, args, env),
    }
}

fn eval_int_prim(op: IntOp, a: i32, b: i32) -> Val {
    match op {
        IntOp::Add => Val::Int(a.wrapping_add(b)),
        IntOp::Sub => Val::Int(a.wrapping_sub(b)),
        IntOp::Mul => Val::Int(a.wrapping_mul(b)),
        IntOp::Eq => Val::Ctor(if a == b { 1 } else { 0 }, vec![]),
        IntOp::Lt => Val::Ctor(if a < b { 1 } else { 0 }, vec![]),
        IntOp::Byte => unreachable!(),
    }
}

fn try_fold_bytes_prim(op: BytesOp, args: &[String], env: &Env) -> Option<Val> {
    match op {
        BytesOp::Len => {
            match env.get(&args[0]) {
                Some(Known::Bytes(bs)) => Some(Val::Int(bs.len() as i32)),
                _ => None,
            }
        }
        BytesOp::Get => {
            match (env.get(&args[0]), env.get(&args[1])) {
                (Some(Known::Bytes(bs)), Some(Known::Int(idx))) => {
                    let idx = *idx as usize;
                    if idx < bs.len() {
                        Some(Val::Int(bs[idx] as i32))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        BytesOp::Concat => {
            match (env.get(&args[0]), env.get(&args[1])) {
                (Some(Known::Bytes(a)), Some(Known::Bytes(b))) => {
                    let mut result = a.clone();
                    result.extend_from_slice(b);
                    Some(Val::Bytes(result))
                }
                _ => None,
            }
        }
        BytesOp::Slice => {
            match (env.get(&args[0]), env.get(&args[1]), env.get(&args[2])) {
                (Some(Known::Bytes(bs)), Some(Known::Int(start)), Some(Known::Int(len))) => {
                    let start = *start as usize;
                    let len = *len as usize;
                    if start + len <= bs.len() {
                        Some(Val::Bytes(bs[start..start + len].to_vec()))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        BytesOp::Eq => {
            match (env.get(&args[0]), env.get(&args[1])) {
                (Some(Known::Bytes(a)), Some(Known::Bytes(b))) => {
                    let tag = if a == b { 1 } else { 0 };
                    Some(Val::Ctor(tag, vec![]))
                }
                _ => None,
            }
        }
    }
}
