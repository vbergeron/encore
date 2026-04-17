// Move loop-invariant bindings out of recursive functions.
// Reduces closure allocations and GC pressure inside hot loops.
//
//   fix loop = n ->               let one = 1 in
//     let one = 1 in              fix loop = n ->
//     builtin add n one    ──►      builtin add n one
//

use std::collections::HashSet;

use crate::ir::cps::{Expr, Fun, Val};
use crate::ir::cps_traversal::CPSTransformer;

pub fn hoisting(expr: Expr) -> Expr {
    Hoisting.transform_expr(&mut (), expr)
}

struct Hoisting;

impl CPSTransformer for Hoisting {
    type Ctx = ();

    fn transform_letrec(&self, ctx: &mut (), name: String, fun: Fun, body: Expr) -> Expr {
        let inner_body = self.transform_expr(ctx, *fun.body);
        let outer_body = self.transform_expr(ctx, body);

        let mut variant = HashSet::new();
        variant.insert(name.clone());
        for a in &fun.args {
            variant.insert(a.clone());
        }
        variant.insert(fun.cont.clone());

        let mut hoisted = Vec::new();
        let remaining = extract_hoistable(inner_body, &mut variant, &mut hoisted);

        let fun = Fun {
            args: fun.args,
            cont: fun.cont,
            body: Box::new(remaining),
        };
        let mut result = Expr::Letrec(name, fun, Box::new(outer_body));
        for (n, v) in hoisted.into_iter().rev() {
            result = Expr::Let(n, v, Box::new(result));
        }
        result
    }
}

fn val_uses_any(val: &Val, names: &HashSet<String>) -> bool {
    match val {
        Val::Var(n) => names.contains(n),
        Val::Cont(_) => true,
        Val::Ctor(_, fields) => fields.iter().any(|f| names.contains(f)),
        Val::Field(n, _) => names.contains(n),
        Val::Int(_) | Val::Bytes(_) | Val::NullCont => false,
        Val::Prim(_, args) => args.iter().any(|a| names.contains(a)),
        Val::Extern(_) => false,
    }
}

/// Walk the Let-chain at the top of a Letrec body.
/// Bindings whose values are loop-invariant get collected into `hoisted`;
/// variant bindings stay in place and their names join the variant set
/// (so later bindings that depend on them are also considered variant).
fn extract_hoistable(
    expr: Expr,
    variant: &mut HashSet<String>,
    hoisted: &mut Vec<(String, Val)>,
) -> Expr {
    match expr {
        Expr::Let(name, val, body) => {
            if val_uses_any(&val, variant) {
                variant.insert(name.clone());
                let body = extract_hoistable(*body, variant, hoisted);
                Expr::Let(name, val, Box::new(body))
            } else {
                hoisted.push((name, val));
                extract_hoistable(*body, variant, hoisted)
            }
        }
        other => other,
    }
}
