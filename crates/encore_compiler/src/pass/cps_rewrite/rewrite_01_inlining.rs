// Duplicate small function bodies at their call sites,
// exposing new redexes for shrinking passes.
//
//   let double = cont(x). builtin add x x
//   in let _nc = nullcont in encore double 3 _nc     ──►   builtin add 3 3
//

use std::collections::HashMap;

use crate::ir::cps::{self, Expr, Fun, Cont, Val};
use crate::pass::cps_subst::subst_expr;

pub fn inlining(expr: Expr, threshold: usize) -> Expr {
    inline_expr(expr, threshold, &HashMap::new())
}

type Env = HashMap<String, Cont>;

fn expr_size(expr: &Expr) -> usize {
    match expr {
        Expr::Let(_, val, body) => 1 + val_size(val) + expr_size(body),
        Expr::Letrec(_, fun, body) => 1 + expr_size(&fun.body) + expr_size(body),
        Expr::Encore(_, _, _) => 1,
        Expr::Match(_, _, cases) => {
            1 + cases.iter().map(|c| expr_size(&c.body)).sum::<usize>()
        }
        Expr::Fin(_) => 1,
    }
}

fn val_size(val: &Val) -> usize {
    match val {
        Val::Cont(cont) => expr_size(&cont.body),
        _ => 1,
    }
}

fn inline_expr(expr: Expr, threshold: usize, env: &Env) -> Expr {
    match expr {
        Expr::Let(name, Val::Cont(cont), body) => {
            let cont = Cont {
                param: cont.param,
                body: Box::new(inline_expr(*cont.body, threshold, env)),
            };
            let mut env = env.clone();
            if expr_size(&cont.body) <= threshold {
                env.insert(name.clone(), cont.clone());
            }
            let body = inline_expr(*body, threshold, &env);
            Expr::Let(name, Val::Cont(cont), Box::new(body))
        }
        Expr::Let(name, val, body) => {
            let val = inline_val(val, threshold, env);
            let body = inline_expr(*body, threshold, env);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, fun, body) => {
            let fun = Fun {
                args: fun.args,
                cont: fun.cont,
                body: Box::new(inline_expr(*fun.body, threshold, env)),
            };
            let body = inline_expr(*body, threshold, env);
            Expr::Letrec(name, fun, Box::new(body))
        }
        Expr::Encore(ref f, ref args, _) => {
            if args.len() == 1 {
                if let Some(cont) = env.get(f) {
                    let mut body = *cont.body.clone();
                    subst_expr(&cont.param, &args[0], &mut body);
                    return body;
                }
            }
            expr
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
        other => other,
    }
}

fn inline_val(val: Val, threshold: usize, env: &Env) -> Val {
    match val {
        Val::Cont(cont) => Val::Cont(Cont {
            param: cont.param,
            body: Box::new(inline_expr(*cont.body, threshold, env)),
        }),
        other => other,
    }
}
