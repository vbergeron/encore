// Duplicate small function bodies at their call sites,
// exposing new redexes for shrinking passes.
//
//   let double = x -> builtin add x x
//   in double 3     ──►   builtin add 3 3
//

use std::collections::HashMap;

use crate::ir::cps::{self, Expr, Lambda, Val};

pub fn inlining(expr: Expr, threshold: usize) -> Expr {
    inline_expr(expr, threshold, &HashMap::new())
}

type Env = HashMap<String, Lambda>;

fn expr_size(expr: &Expr) -> usize {
    match expr {
        Expr::Let(_, val, body) => 1 + val_size(val) + expr_size(body),
        Expr::Letrec(_, lam, body) => 1 + expr_size(&lam.body) + expr_size(body),
        Expr::App(_, _) => 1,
        Expr::Match(_, _, cases) => {
            1 + cases.iter().map(|c| expr_size(&c.body)).sum::<usize>()
        }
        Expr::Fin(_) => 1,
    }
}

fn val_size(val: &Val) -> usize {
    match val {
        Val::Lambda(lam) => expr_size(&lam.body),
        _ => 1,
    }
}

fn inline_expr(expr: Expr, threshold: usize, env: &Env) -> Expr {
    match expr {
        Expr::Let(name, Val::Lambda(lam), body) => {
            let lam = Lambda {
                param: lam.param,
                body: Box::new(inline_expr(*lam.body, threshold, env)),
            };
            let mut env = env.clone();
            if expr_size(&lam.body) <= threshold {
                env.insert(name.clone(), lam.clone());
            }
            let body = inline_expr(*body, threshold, &env);
            Expr::Let(name, Val::Lambda(lam), Box::new(body))
        }
        Expr::Let(name, val, body) => {
            let val = inline_val(val, threshold, env);
            let body = inline_expr(*body, threshold, env);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, lam, body) => {
            let lam = Lambda {
                param: lam.param,
                body: Box::new(inline_expr(*lam.body, threshold, env)),
            };
            let body = inline_expr(*body, threshold, env);
            Expr::Letrec(name, lam, Box::new(body))
        }
        Expr::App(f, x) => {
            if let Some(lam) = env.get(&f) {
                let mut body = *lam.body.clone();
                subst_expr(&lam.param, &x, &mut body);
                body
            } else {
                Expr::App(f, x)
            }
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case {
                    binds: c.binds,
                    body: inline_expr(c.body, threshold, env),
                })
                .collect();
            Expr::Match(name, base, cases)
        }
        Expr::Fin(n) => Expr::Fin(n),
    }
}

fn inline_val(val: Val, threshold: usize, env: &Env) -> Val {
    match val {
        Val::Lambda(lam) => Val::Lambda(Lambda {
            param: lam.param,
            body: Box::new(inline_expr(*lam.body, threshold, env)),
        }),
        other => other,
    }
}

fn subst_name(from: &str, to: &str, name: &mut String) {
    if name == from {
        *name = to.to_string();
    }
}

fn subst_val(from: &str, to: &str, val: &mut Val) {
    match val {
        Val::Var(n) => subst_name(from, to, n),
        Val::Lambda(lam) => {
            if lam.param != from {
                subst_expr(from, to, &mut lam.body);
            }
        }
        Val::Ctor(_, fields) => {
            for f in fields {
                subst_name(from, to, f);
            }
        }
        Val::Field(n, _) => subst_name(from, to, n),
        Val::Int(_) => {}
        Val::Prim(_, args) => {
            for a in args {
                subst_name(from, to, a);
            }
        }
    }
}

fn subst_expr(from: &str, to: &str, expr: &mut Expr) {
    match expr {
        Expr::Let(binder, val, body) => {
            subst_val(from, to, val);
            if binder != from {
                subst_expr(from, to, body);
            }
        }
        Expr::Letrec(binder, lam, body) => {
            if binder != from {
                if lam.param != from {
                    subst_expr(from, to, &mut lam.body);
                }
                subst_expr(from, to, body);
            }
        }
        Expr::App(f, x) => {
            subst_name(from, to, f);
            subst_name(from, to, x);
        }
        Expr::Match(n, _, cases) => {
            subst_name(from, to, n);
            for case in cases {
                if !case.binds.contains(&from.to_string()) {
                    subst_expr(from, to, &mut case.body);
                }
            }
        }
        Expr::Fin(n) => {
            subst_name(from, to, n);
        }
    }
}
