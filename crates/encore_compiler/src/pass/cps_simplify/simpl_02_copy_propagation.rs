// Replace a name that is just an alias for another with the original.
//
//   let y = x in f y     ──►   f x
//

use crate::ir::cps::{Expr, Val};
use crate::ir::cps_traversal::CPSTransformer;

use super::subst_expr;

pub fn copy_propagation(expr: Expr) -> Expr {
    CopyPropagation.transform_expr(&mut (), expr)
}

struct CopyPropagation;

impl CPSTransformer for CopyPropagation {
    type Ctx = ();

    fn transform_let(&self, ctx: &mut (), name: String, val: Val, body: Expr) -> Expr {
        if let Val::Var(y) = val {
            let mut body = self.transform_expr(ctx, body);
            subst_expr(&name, &y, &mut body);
            body
        } else {
            Expr::Let(
                name,
                self.transform_val(ctx, val),
                Box::new(self.transform_expr(ctx, body)),
            )
        }
    }
}
