// Evaluate primitive operations on known integer constants at compile time.
//
//   let x = 3 in let y = 4 in builtin add x y     ──►   7
//

use std::collections::HashMap;

use crate::ir::cps::{self, Expr, Lambda, Val};
use crate::ir::prim::PrimOp;

pub fn constant_fold(expr: Expr) -> Expr {
    constant_fold_env(expr, &mut HashMap::new())
}

fn constant_fold_env(expr: Expr, env: &mut HashMap<String, i32>) -> Expr {
    match expr {
        Expr::Let(name, Val::Int(n), body) => {
            env.insert(name.clone(), n);
            let body = constant_fold_env(*body, env);
            Expr::Let(name, Val::Int(n), Box::new(body))
        }
        Expr::Let(name, Val::Prim(op, ref args), body) => {
            if let (Some(&a), Some(&b)) = (env.get(&args[0]), env.get(&args[1])) {
                if let Some(result) = eval_prim(op, a, b) {
                    env.insert(name.clone(), result);
                    let body = constant_fold_env(*body, env);
                    return Expr::Let(name, Val::Int(result), Box::new(body));
                }
            }
            {
                let val = Val::Prim(op, args.clone());
                let body = constant_fold_env(*body, env);
                Expr::Let(name, val, Box::new(body))
            }
        }
        Expr::Let(name, val, body) => {
            let val = constant_fold_val(val, env);
            let body = constant_fold_env(*body, env);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, lam, body) => {
            let lam = constant_fold_lambda(lam, env);
            let body = constant_fold_env(*body, env);
            Expr::Letrec(name, lam, Box::new(body))
        }
        Expr::Match(name, base, cases) => {
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

fn constant_fold_val(val: Val, env: &mut HashMap<String, i32>) -> Val {
    match val {
        Val::Lambda(lam) => Val::Lambda(constant_fold_lambda(lam, env)),
        other => other,
    }
}

fn constant_fold_lambda(lam: Lambda, env: &mut HashMap<String, i32>) -> Lambda {
    Lambda {
        param: lam.param,
        body: Box::new(constant_fold_env(*lam.body, &mut env.clone())),
    }
}

fn eval_prim(op: PrimOp, a: i32, b: i32) -> Option<i32> {
    match op {
        PrimOp::Add => Some(a.wrapping_add(b)),
        PrimOp::Sub => Some(a.wrapping_sub(b)),
        PrimOp::Mul => Some(a.wrapping_mul(b)),
        PrimOp::Eq | PrimOp::Lt => None,
    }
}
