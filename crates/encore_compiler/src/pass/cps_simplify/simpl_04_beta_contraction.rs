// Inline a continuation that is called exactly once, removing the closure allocation.
// The CPS transform produces an administrative continuation per application;
// nearly all of them are single-use and this pass eliminates them.
//
//   let k = cont(x). x in let _nc = nullcont in encore k arg _nc     ──►   arg
//

use crate::ir::cps::{self, Cont, Expr, Val};
use crate::ir::cps_traversal::CPSTransformer;

use super::{Census, census_expr, count, subst_expr};

pub fn beta_contraction(expr: Expr) -> Expr {
    BetaContraction.transform_expr(&mut (), expr)
}

struct BetaContraction;

impl CPSTransformer for BetaContraction {
    type Ctx = ();

    fn transform_let(&self, ctx: &mut (), name: String, val: Val, body: Expr) -> Expr {
        if let Val::Cont(cont) = val {
            let body = self.transform_expr(ctx, body);
            let mut census = Census::new();
            census_expr(&mut census, &body);
            if count(&census, &name) == 1 {
                if let Some(inlined) = try_inline(&name, &cont, body.clone()) {
                    return inlined;
                }
            }
            let cont = self.transform_cont(ctx, cont);
            Expr::Let(name, Val::Cont(cont), Box::new(body))
        } else {
            Expr::Let(
                name,
                self.transform_val(ctx, val),
                Box::new(self.transform_expr(ctx, body)),
            )
        }
    }
}

fn try_inline(name: &str, cont: &Cont, expr: Expr) -> Option<Expr> {
    match expr {
        Expr::Let(_, Val::NullCont, ref body) => {
            if let Expr::Encore(ref k, ref args, _) = **body {
                if k == name && args.len() == cont.params.len() {
                    let mut result = *cont.body.clone();
                    for (param, arg) in cont.params.iter().zip(args.iter()) {
                        subst_expr(param, arg, &mut result);
                    }
                    return Some(result);
                }
            }
            None
        }
        Expr::Let(n, val, body) => {
            try_inline(name, cont, *body).map(|new_body| Expr::Let(n, val, Box::new(new_body)))
        }
        Expr::Letrec(n, f, body) => {
            try_inline(name, cont, *body).map(|new_body| Expr::Letrec(n, f, Box::new(new_body)))
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
