// Replace a name that is just an alias for another with the original.
//
//   let y = x in f y     ──►   f x
//

use crate::ir::cps::{self, Expr, Lambda, Val};

use super::subst_expr;

pub fn copy_propagation(expr: Expr) -> Expr {
    match expr {
        Expr::Let(name, Val::Var(y), body) => {
            let mut body = copy_propagation(*body);
            subst_expr(&name, &y, &mut body);
            body
        }
        Expr::Let(name, val, body) => {
            let val = copy_propagation_val(val);
            let body = copy_propagation(*body);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, lam, body) => {
            let lam = copy_propagation_lambda(lam);
            let body = copy_propagation(*body);
            Expr::Letrec(name, lam, Box::new(body))
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case { binds: c.binds, body: copy_propagation(c.body) })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn copy_propagation_val(val: Val) -> Val {
    match val {
        Val::Lambda(lam) => Val::Lambda(copy_propagation_lambda(lam)),
        other => other,
    }
}

fn copy_propagation_lambda(lam: Lambda) -> Lambda {
    Lambda { param: lam.param, body: Box::new(copy_propagation(*lam.body)) }
}
