// Reuse a previously computed value instead of recomputing it.
//
//   let a = field 0 of x in       let a = field 0 of x in
//   let b = field 0 of x in       ... a ... a ...
//   ... a ... b ...         ──►
//

use crate::ir::cps::{self, Cont, Expr, Fun, Val};
use crate::pass::cps_subst::subst_expr;

pub fn cse(expr: Expr) -> Expr {
    cse_expr(expr, &Vec::new())
}

type Available = Vec<(Val, String)>;

fn is_cse_candidate(val: &Val) -> bool {
    matches!(val, Val::Field(_, _) | Val::Prim(_, _) | Val::Ctor(_, _))
}

fn val_mentions(val: &Val, name: &str) -> bool {
    match val {
        Val::Var(n) => n == name,
        Val::Cont(_) => false,
        Val::Ctor(_, fields) => fields.iter().any(|f| f == name),
        Val::Field(n, _) => n == name,
        Val::Int(_) | Val::NullCont => false,
        Val::Prim(_, args) => args.iter().any(|a| a == name),
        Val::Extern(_) => false,
    }
}

fn find_available(avail: &Available, val: &Val) -> Option<String> {
    if !is_cse_candidate(val) {
        return None;
    }
    avail
        .iter()
        .rev()
        .find(|(v, _)| v == val)
        .map(|(_, n)| n.clone())
}

/// Remove entries whose val or bound name mentions any of the given names.
fn invalidate(avail: &Available, shadows: &[&str]) -> Available {
    avail
        .iter()
        .filter(|(v, n)| {
            !shadows
                .iter()
                .any(|s| n == s || val_mentions(v, s))
        })
        .cloned()
        .collect()
}

fn cse_expr(expr: Expr, avail: &Available) -> Expr {
    match expr {
        Expr::Let(name, val, body) => {
            let val = cse_val(val, avail);
            if let Some(existing) = find_available(avail, &val) {
                let mut body = *body;
                subst_expr(&name, &existing, &mut body);
                cse_expr(body, avail)
            } else {
                let mut avail = invalidate(avail, &[&name]);
                avail.push((val.clone(), name.clone()));
                let body = cse_expr(*body, &avail);
                Expr::Let(name, val, Box::new(body))
            }
        }
        Expr::Letrec(name, fun, body) => {
            let fun = Fun {
                args: fun.args,
                cont: fun.cont,
                body: Box::new(cse_expr(*fun.body, &Vec::new())),
            };
            let body = cse_expr(*body, avail);
            Expr::Letrec(name, fun, Box::new(body))
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| {
                    let refs: Vec<&str> = c.binds.iter().map(|s| s.as_str()).collect();
                    let branch_avail = invalidate(avail, &refs);
                    cps::Case {
                        binds: c.binds,
                        body: cse_expr(c.body, &branch_avail),
                    }
                })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn cse_val(val: Val, avail: &Available) -> Val {
    match val {
        Val::Cont(cont) => Val::Cont(Cont {
            param: cont.param,
            body: Box::new(cse_expr(*cont.body, avail)),
        }),
        other => other,
    }
}
