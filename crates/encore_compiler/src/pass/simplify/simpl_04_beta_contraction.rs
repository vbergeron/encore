// Inline a continuation that is called exactly once, removing the closure allocation.
// The CPS transform produces an administrative continuation per application;
// nearly all of them are single-use and this pass eliminates them.
//
//   let k = cont(x). x in return k arg     ──►   arg
//

use crate::ir::cps::{self, Expr, Fun, Cont, Val};

use super::{count, census_expr, subst_expr, Census};

pub fn beta_contraction(expr: Expr) -> Expr {
    match expr {
        Expr::Let(name, Val::Cont(cont), body) => {
            let body = beta_contraction(*body);
            let mut census = Census::new();
            census_expr(&mut census, &body);
            if count(&census, &name) == 1 {
                if let Some(inlined) = try_inline(&name, &cont, body.clone()) {
                    return inlined;
                }
            }
            let cont = beta_contraction_cont(cont);
            Expr::Let(name, Val::Cont(cont), Box::new(body))
        }
        Expr::Let(name, val, body) => {
            let val = beta_contraction_val(val);
            let body = beta_contraction(*body);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, fun, body) => {
            let fun = beta_contraction_fun(fun);
            let body = beta_contraction(*body);
            Expr::Letrec(name, fun, Box::new(body))
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
        Val::Cont(cont) => Val::Cont(beta_contraction_cont(cont)),
        other => other,
    }
}

fn beta_contraction_fun(fun: Fun) -> Fun {
    Fun { arg: fun.arg, cont: fun.cont, body: Box::new(beta_contraction(*fun.body)) }
}

fn beta_contraction_cont(cont: Cont) -> Cont {
    Cont { param: cont.param, body: Box::new(beta_contraction(*cont.body)) }
}

/// Walk the continuation looking for `Return(name, arg)` and replace it
/// with the cont body where `param := arg`.
fn try_inline(name: &str, cont: &Cont, expr: Expr) -> Option<Expr> {
    match expr {
        Expr::Return(ref k, ref x) if k == name => {
            let mut body = *cont.body.clone();
            subst_expr(&cont.param, x, &mut body);
            Some(body)
        }
        Expr::Let(n, val, body) => {
            try_inline(name, cont, *body)
                .map(|new_body| Expr::Let(n, val, Box::new(new_body)))
        }
        Expr::Letrec(n, f, body) => {
            try_inline(name, cont, *body)
                .map(|new_body| Expr::Letrec(n, f, Box::new(new_body)))
        }
        Expr::Match(n, base, cases) => {
            let mut found = false;
            let new_cases: Vec<_> = cases
                .into_iter()
                .map(|c| {
                    if found {
                        return c;
                    }
                    if let Some(new_body) = try_inline(name, cont, c.body.clone()) {
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
