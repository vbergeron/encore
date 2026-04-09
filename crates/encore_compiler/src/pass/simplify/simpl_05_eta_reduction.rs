// Collapse a lambda that just forwards its argument to another function.
//
//   let g = x -> f x in g arg     ──►   f arg
//

use crate::ir::cps::{self, Expr, Lambda, Val};

pub fn eta_reduction(expr: Expr) -> Expr {
    match expr {
        Expr::Let(name, Val::Lambda(lam), body) => {
            let body = eta_reduction(*body);
            if let Expr::App(ref f, ref x) = *lam.body {
                if *x == lam.param && *f != lam.param {
                    return Expr::Let(name, Val::Var(f.clone()), Box::new(body));
                }
            }
            Expr::Let(name, Val::Lambda(eta_reduction_lambda(lam)), Box::new(body))
        }
        Expr::Let(name, val, body) => {
            let val = eta_reduction_val(val);
            let body = eta_reduction(*body);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, lam, body) => {
            let lam = eta_reduction_lambda(lam);
            let body = eta_reduction(*body);
            Expr::Letrec(name, lam, Box::new(body))
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case { binds: c.binds, body: eta_reduction(c.body) })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn eta_reduction_val(val: Val) -> Val {
    match val {
        Val::Lambda(lam) => Val::Lambda(eta_reduction_lambda(lam)),
        other => other,
    }
}

fn eta_reduction_lambda(lam: Lambda) -> Lambda {
    Lambda { param: lam.param, body: Box::new(eta_reduction(*lam.body)) }
}
