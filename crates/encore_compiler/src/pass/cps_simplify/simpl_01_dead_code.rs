// Drop bindings whose name is never referenced in the body.
//
//   let unused = Succ(x) in unused     ──►   unused
//

use crate::ir::cps::{Expr, Fun, Val};
use crate::ir::cps_traversal::CPSTransformer;

use super::{Census, census_expr, count, is_pure};

pub fn dead_code(expr: Expr) -> Expr {
    DeadCode.transform_expr(&mut (), expr)
}

struct DeadCode;

impl CPSTransformer for DeadCode {
    type Ctx = ();

    fn transform_let(&self, ctx: &mut (), name: String, val: Val, body: Expr) -> Expr {
        let val = self.transform_val(ctx, val);
        let body = self.transform_expr(ctx, body);
        let mut census = Census::new();
        census_expr(&mut census, &body);
        if count(&census, &name) == 0 && is_pure(&val) {
            body
        } else {
            Expr::Let(name, val, Box::new(body))
        }
    }

    fn transform_letrec(&self, ctx: &mut (), name: String, fun: Fun, body: Expr) -> Expr {
        let fun = self.transform_fun(ctx, fun);
        let body = self.transform_expr(ctx, body);
        let mut census = Census::new();
        census_expr(&mut census, &body);
        if count(&census, &name) == 0 {
            body
        } else {
            Expr::Letrec(name, fun, Box::new(body))
        }
    }
}
