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
            if cont.param != from {
                subst_expr(from, to, &mut cont.body);
            }
        }
        Val::Ctor(_, fields) => {
            for f in fields {
                subst_name(from, to, f);
            }
        }
        Val::Field(n, _) => subst_name(from, to, n),
        Val::Int(_) => {}
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
                if fun.arg != from && fun.cont != from {
                    subst_expr(from, to, &mut fun.body);
                }
                subst_expr(from, to, body);
            }
        }
        Expr::Encore(f, x, k) => {
            subst_name(from, to, f);
            subst_name(from, to, x);
            subst_name(from, to, k);
        }
        Expr::Return(k, x) => {
            subst_name(from, to, k);
            subst_name(from, to, x);
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
