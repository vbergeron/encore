// Collapse a continuation that just forwards its argument to another continuation.
//
//   let g = cont(x). let _nc = nullcont in encore f x _nc
//   in let _nc2 = nullcont in encore g arg _nc2
//   ──►   let g = var(f) in let _nc2 = nullcont in encore g arg _nc2
//

use crate::ir::cps::{Expr, Val};
use crate::ir::cps_traversal::CPSTransformer;

pub fn eta_reduction(expr: Expr) -> Expr {
    EtaReduction.transform_expr(&mut (), expr)
}

struct EtaReduction;

impl CPSTransformer for EtaReduction {
    type Ctx = ();

    fn transform_let(&self, ctx: &mut (), name: String, val: Val, body: Expr) -> Expr {
        if let Val::Cont(cont) = val {
            let body = self.transform_expr(ctx, body);
            if let Expr::Let(_, Val::NullCont, ref inner) = *cont.body {
                if let Expr::Encore(ref f, ref args, _) = **inner {
                    if args.len() == cont.params.len()
                        && args.iter().zip(cont.params.iter()).all(|(a, p)| a == p)
                        && !cont.params.iter().any(|p| p == f)
                    {
                        return Expr::Let(name, Val::Var(f.clone()), Box::new(body));
                    }
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
