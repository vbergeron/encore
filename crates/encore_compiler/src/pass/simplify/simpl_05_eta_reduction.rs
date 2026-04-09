// Collapse a continuation that just forwards its argument to another continuation.
//
//   let g = cont(x). return f x in return g arg     ──►   return f arg
//

use crate::ir::cps::{self, Expr, Fun, Cont, Val};

pub fn eta_reduction(expr: Expr) -> Expr {
    match expr {
        Expr::Let(name, Val::Cont(cont), body) => {
            let body = eta_reduction(*body);
            if let Expr::Return(ref f, ref x) = *cont.body {
                if *x == cont.param && *f != cont.param {
                    return Expr::Let(name, Val::Var(f.clone()), Box::new(body));
                }
            }
            Expr::Let(name, Val::Cont(eta_reduction_cont(cont)), Box::new(body))
        }
        Expr::Let(name, val, body) => {
            let val = eta_reduction_val(val);
            let body = eta_reduction(*body);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, fun, body) => {
            let fun = eta_reduction_fun(fun);
            let body = eta_reduction(*body);
            Expr::Letrec(name, fun, Box::new(body))
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
        Val::Cont(cont) => Val::Cont(eta_reduction_cont(cont)),
        other => other,
    }
}

fn eta_reduction_fun(fun: Fun) -> Fun {
    Fun { arg: fun.arg, cont: fun.cont, body: Box::new(eta_reduction(*fun.body)) }
}

fn eta_reduction_cont(cont: Cont) -> Cont {
    Cont { param: cont.param, body: Box::new(eta_reduction(*cont.body)) }
}
