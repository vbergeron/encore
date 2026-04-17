// Reuse a previously computed value instead of recomputing it.
//
//   let a = field 0 of x in       let a = field 0 of x in
//   let b = field 0 of x in       ... a ... a ...
//   ... a ... b ...         ──►
//

use crate::ir::cps::{Case, Cont, Expr, Fun, Tag, Val};
use crate::ir::cps_traversal::CPSTransformer;
use crate::pass::cps_subst::subst_expr;

pub fn cse(expr: Expr) -> Expr {
    Cse.transform_expr(&mut Available::new(), expr)
}

type Available = Vec<(Val, String)>;

struct Cse;

fn is_cse_candidate(val: &Val) -> bool {
    matches!(val, Val::Field(_, _) | Val::Prim(_, _) | Val::Ctor(_, _))
}

fn val_mentions(val: &Val, name: &str) -> bool {
    match val {
        Val::Var(n) => n == name,
        Val::Cont(_) => false,
        Val::Ctor(_, fields) => fields.iter().any(|f| f == name),
        Val::Field(n, _) => n == name,
        Val::Int(_) | Val::Bytes(_) | Val::NullCont => false,
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

/// Drop entries whose val or bound name mentions any of the given names.
fn invalidate_in_place(avail: &mut Available, shadows: &[&str]) {
    avail.retain(|(v, n)| {
        !shadows
            .iter()
            .any(|s| n == s || val_mentions(v, s))
    });
}

impl CPSTransformer for Cse {
    type Ctx = Available;

    fn transform_let(&self, avail: &mut Available, name: String, val: Val, body: Expr) -> Expr {
        let val = self.transform_val(avail, val);
        if let Some(existing) = find_available(avail, &val) {
            let mut body = body;
            subst_expr(&name, &existing, &mut body);
            self.transform_expr(avail, body)
        } else {
            invalidate_in_place(avail, &[&name]);
            avail.push((val.clone(), name.clone()));
            let body = self.transform_expr(avail, body);
            Expr::Let(name, val, Box::new(body))
        }
    }

    fn transform_letrec(&self, avail: &mut Available, name: String, fun: Fun, body: Expr) -> Expr {
        let fun = Fun {
            args: fun.args,
            cont: fun.cont,
            body: Box::new(self.transform_expr(&mut Available::new(), *fun.body)),
        };
        let body = self.transform_expr(avail, body);
        Expr::Letrec(name, fun, Box::new(body))
    }

    fn transform_cont(&self, avail: &mut Available, cont: Cont) -> Cont {
        let mut local = avail.clone();
        Cont {
            params: cont.params,
            body: Box::new(self.transform_expr(&mut local, *cont.body)),
        }
    }

    fn transform_match_expr(
        &self,
        avail: &mut Available,
        scrutinee: String,
        base: Tag,
        cases: Vec<Case>,
    ) -> Expr {
        let cases = cases
            .into_iter()
            .map(|c| {
                let refs: Vec<&str> = c.binds.iter().map(|s| s.as_str()).collect();
                let mut branch = avail.clone();
                invalidate_in_place(&mut branch, &refs);
                Case {
                    binds: c.binds,
                    body: self.transform_expr(&mut branch, c.body),
                }
            })
            .collect();
        Expr::Match(scrutinee, base, cases)
    }
}
