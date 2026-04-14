use std::collections::HashMap;

use crate::ir::cps::{Cont, Expr, Fun, Val};

pub type Census = HashMap<String, usize>;

pub fn census_name(census: &mut Census, name: &str) {
    *census.entry(name.to_string()).or_insert(0) += 1;
}

pub fn census_val(census: &mut Census, val: &Val) {
    match val {
        Val::Var(n) => census_name(census, n),
        Val::Cont(cont) => census_cont(census, cont),
        Val::Ctor(_, fields) => {
            for f in fields {
                census_name(census, f);
            }
        }
        Val::Field(n, _) => census_name(census, n),
        Val::Int(_) | Val::NullCont => {}
        Val::Prim(_, args) => {
            for a in args {
                census_name(census, a);
            }
        }
        Val::Extern(_) => {}
    }
}

pub fn census_fun(census: &mut Census, fun: &Fun) {
    census_expr(census, &fun.body);
}

pub fn census_cont(census: &mut Census, cont: &Cont) {
    census_expr(census, &cont.body);
}

pub fn census_expr(census: &mut Census, expr: &Expr) {
    match expr {
        Expr::Let(_, val, body) => {
            census_val(census, val);
            census_expr(census, body);
        }
        Expr::Letrec(_, fun, body) => {
            census_fun(census, fun);
            census_expr(census, body);
        }
        Expr::Encore(f, args, k) => {
            census_name(census, f);
            for a in args {
                census_name(census, a);
            }
            census_name(census, k);
        }
        Expr::Match(n, _, cases) => {
            census_name(census, n);
            for case in cases {
                census_expr(census, &case.body);
            }
        }
        Expr::Fin(n) => {
            census_name(census, n);
        }
    }
}

pub fn count(census: &Census, name: &str) -> usize {
    census.get(name).copied().unwrap_or(0)
}

pub fn is_pure(val: &Val) -> bool {
    match val {
        Val::Var(_) | Val::Int(_) | Val::NullCont | Val::Cont(_) | Val::Extern(_) => true,
        Val::Ctor(_, _) | Val::Field(_, _) | Val::Prim(_, _) => true,
    }
}
