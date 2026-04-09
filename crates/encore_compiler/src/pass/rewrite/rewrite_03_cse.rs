// Reuse a previously computed value instead of recomputing it.
//
//   let a = field 0 of x in       let a = field 0 of x in
//   let b = field 0 of x in       ... a ... a ...
//   ... a ... b ...         ──►
//

use crate::ir::cps::Expr;

pub fn cse(expr: Expr) -> Expr {
    // TODO
    expr
}
