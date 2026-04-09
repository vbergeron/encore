// Drop bindings whose name is never referenced in the body.
//
//   let unused = Succ(x) in unused     ──►   unused
//

use crate::ir::cps::{self, Expr, Lambda, Val};

use super::{count, census_expr, is_pure, Census};

pub fn dead_code(expr: Expr) -> Expr {
    match expr {
        Expr::Let(name, val, body) => {
            let body = dead_code(*body);
            let val = dead_code_val(val);
            let mut census = Census::new();
            census_expr(&mut census, &body);
            if count(&census, &name) == 0 && is_pure(&val) {
                body
            } else {
                Expr::Let(name, val, Box::new(body))
            }
        }
        Expr::Letrec(name, lam, body) => {
            let body = dead_code(*body);
            let lam = dead_code_lambda(lam);
            let mut census = Census::new();
            census_expr(&mut census, &body);
            if count(&census, &name) == 0 {
                body
            } else {
                Expr::Letrec(name, lam, Box::new(body))
            }
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case { binds: c.binds, body: dead_code(c.body) })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn dead_code_val(val: Val) -> Val {
    match val {
        Val::Lambda(lam) => Val::Lambda(dead_code_lambda(lam)),
        other => other,
    }
}

fn dead_code_lambda(lam: Lambda) -> Lambda {
    Lambda { param: lam.param, body: Box::new(dead_code(*lam.body)) }
}
