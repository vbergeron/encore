// Move loop-invariant bindings out of recursive functions.
// Reduces closure allocations and GC pressure inside hot loops.
//
//   fix loop = n ->               let one = 1 in
//     let one = 1 in              fix loop = n ->
//     builtin add n one    ──►      builtin add n one
//

use std::collections::HashSet;

use crate::ir::cps::{self, Cont, Expr, Fun, Val};

pub fn hoisting(expr: Expr) -> Expr {
    hoist_expr(expr)
}

fn hoist_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Letrec(name, fun, body) => {
            let inner_body = hoist_expr(*fun.body);
            let outer_body = hoist_expr(*body);

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
        Expr::Let(name, val, body) => {
            let val = hoist_val(val);
            let body = hoist_expr(*body);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case {
                    binds: c.binds,
                    body: hoist_expr(c.body),
                })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn hoist_val(val: Val) -> Val {
    match val {
        Val::Cont(cont) => Val::Cont(Cont {
            params: cont.params,
            body: Box::new(hoist_expr(*cont.body)),
        }),
        other => other,
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
