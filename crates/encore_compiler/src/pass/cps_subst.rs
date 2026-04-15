use crate::ir::cps::{Expr, Val};

pub fn subst_name(from: &str, to: &str, name: &mut String) {
    if name == from {
        *name = to.to_string();
    }
}

pub fn subst_val(from: &str, to: &str, val: &mut Val) {
    match val {
        Val::Var(n) => subst_name(from, to, n),
        Val::Cont(cont) => {
            if !cont.params.iter().any(|p| p == from) {
                subst_expr(from, to, &mut cont.body);
            }
        }
        Val::Ctor(_, fields) => {
            for f in fields {
                subst_name(from, to, f);
            }
        }
        Val::Field(n, _) => subst_name(from, to, n),
        Val::Int(_) | Val::Bytes(_) | Val::NullCont => {}
        Val::Prim(_, args) => {
            for a in args {
                subst_name(from, to, a);
            }
        }
        Val::Extern(_) => {}
    }
}

pub fn subst_expr(from: &str, to: &str, expr: &mut Expr) {
    match expr {
        Expr::Let(binder, val, body) => {
            subst_val(from, to, val);
            if binder != from {
                subst_expr(from, to, body);
            }
        }
        Expr::Letrec(binder, fun, body) => {
            if binder != from {
                if !fun.args.contains(&from.to_string()) && fun.cont != from {
                    subst_expr(from, to, &mut fun.body);
                }
                subst_expr(from, to, body);
            }
        }
        Expr::Encore(f, args, k) => {
            subst_name(from, to, f);
            for a in args {
                subst_name(from, to, a);
            }
            subst_name(from, to, k);
        }
        Expr::Match(n, _, cases) => {
            subst_name(from, to, n);
            for case in cases {
                if !case.binds.contains(&from.to_string()) {
                    subst_expr(from, to, &mut case.body);
                }
            }
        }
        Expr::Fin(n) => {
            subst_name(from, to, n);
        }
    }
}
