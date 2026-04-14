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

use std::collections::HashMap;

use crate::ir::cps::{self, Expr, Fun, Cont, Val, Tag};
use crate::ir::prim::PrimOp;
use crate::pass::cps_subst::subst_expr;

#[derive(Clone)]
enum Known {
    Int(i32),
    Ctor(Tag, Vec<String>),
}

type Env = HashMap<String, Known>;

pub fn constant_fold(expr: Expr) -> Expr {
    constant_fold_env(expr, &mut Env::new())
}

fn record(env: &mut Env, name: &str, val: &Val) {
    match val {
        Val::Int(n) => { env.insert(name.to_string(), Known::Int(*n)); }
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
            let folded = match (env.get(&args[0]), env.get(&args[1])) {
                (Some(Known::Int(a)), Some(Known::Int(b))) => Some(eval_prim(op, *a, *b)),
                _ => None,
            };
            if let Some(val) = folded {
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
        param: cont.param,
        body: Box::new(constant_fold_env(*cont.body, &mut env.clone())),
    }
}

fn eval_prim(op: PrimOp, a: i32, b: i32) -> Val {
    match op {
        PrimOp::Add => Val::Int(a.wrapping_add(b)),
        PrimOp::Sub => Val::Int(a.wrapping_sub(b)),
        PrimOp::Mul => Val::Int(a.wrapping_mul(b)),
        PrimOp::Eq => Val::Ctor(if a == b { 1 } else { 0 }, vec![]),
        PrimOp::Lt => Val::Ctor(if a < b { 1 } else { 0 }, vec![]),
    }
}
