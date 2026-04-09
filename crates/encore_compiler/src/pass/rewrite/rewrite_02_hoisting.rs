// Move loop-invariant bindings out of recursive functions.
// Reduces closure allocations and GC pressure inside hot loops.
//
//   fix loop = n ->               let one = 1 in
//     let one = 1 in              fix loop = n ->
//     builtin add n one    ──►      builtin add n one
//

use crate::ir::cps::Expr;

pub fn hoisting(expr: Expr) -> Expr {
    // TODO
    expr
}
