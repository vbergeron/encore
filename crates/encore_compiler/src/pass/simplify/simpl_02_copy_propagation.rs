// Replace a name that is just an alias for another with the original.
//
//   let y = x in f y     ──►   f x
//

use crate::ir::cps::{self, Expr, Fun, Cont, Val};

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
        Expr::Letrec(name, fun, body) => {
            let fun = copy_propagation_fun(fun);
            let body = copy_propagation(*body);
            Expr::Letrec(name, fun, Box::new(body))
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
        Val::Cont(cont) => Val::Cont(copy_propagation_cont(cont)),
        other => other,
    }
}

fn copy_propagation_fun(fun: Fun) -> Fun {
    Fun { arg: fun.arg, cont: fun.cont, body: Box::new(copy_propagation(*fun.body)) }
}

fn copy_propagation_cont(cont: Cont) -> Cont {
    Cont { param: cont.param, body: Box::new(copy_propagation(*cont.body)) }
}
