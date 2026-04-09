// Inline a lambda that is called exactly once, removing the closure allocation.
// The CPS transform produces an administrative continuation per application;
// nearly all of them are single-use and this pass eliminates them.
//
//   let k = x -> x in k arg     ──►   arg
//

use crate::ir::cps::{self, Expr, Lambda, Val};

use super::{count, census_expr, subst_expr, Census};

pub fn beta_contraction(expr: Expr) -> Expr {
    match expr {
        Expr::Let(name, Val::Lambda(lam), body) => {
            let body = beta_contraction(*body);
            let mut census = Census::new();
            census_expr(&mut census, &body);
            if count(&census, &name) == 1 {
                if let Some(inlined) = try_inline(&name, &lam, body.clone()) {
                    return inlined;
                }
            }
            let lam = beta_contraction_lambda(lam);
            Expr::Let(name, Val::Lambda(lam), Box::new(body))
        }
        Expr::Let(name, val, body) => {
            let val = beta_contraction_val(val);
            let body = beta_contraction(*body);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, lam, body) => {
            let lam = beta_contraction_lambda(lam);
            let body = beta_contraction(*body);
            Expr::Letrec(name, lam, Box::new(body))
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case { binds: c.binds, body: beta_contraction(c.body) })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn beta_contraction_val(val: Val) -> Val {
    match val {
        Val::Lambda(lam) => Val::Lambda(beta_contraction_lambda(lam)),
        other => other,
    }
}

fn beta_contraction_lambda(lam: Lambda) -> Lambda {
    Lambda { param: lam.param, body: Box::new(beta_contraction(*lam.body)) }
}

/// Walk the continuation looking for `App(name, arg)` and replace it
/// with the lambda body where `param := arg`.
fn try_inline(name: &str, lam: &Lambda, expr: Expr) -> Option<Expr> {
    match expr {
        Expr::App(ref f, ref x) if f == name => {
            let mut body = *lam.body.clone();
            subst_expr(&lam.param, x, &mut body);
            Some(body)
        }
        Expr::Let(n, val, body) => {
            try_inline(name, lam, *body)
                .map(|new_body| Expr::Let(n, val, Box::new(new_body)))
        }
        Expr::Letrec(n, l, body) => {
            try_inline(name, lam, *body)
                .map(|new_body| Expr::Letrec(n, l, Box::new(new_body)))
        }
        Expr::Match(n, base, cases) => {
            let mut found = false;
            let new_cases: Vec<_> = cases
                .into_iter()
                .map(|c| {
                    if found {
                        return c;
                    }
                    if let Some(new_body) = try_inline(name, lam, c.body.clone()) {
                        found = true;
                        cps::Case { binds: c.binds, body: new_body }
                    } else {
                        c
                    }
                })
                .collect();
            if found { Some(Expr::Match(n, base, new_cases)) } else { None }
        }
        _ => None,
    }
}
