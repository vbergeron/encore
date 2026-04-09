mod simpl_01_dead_code;
mod simpl_02_copy_propagation;
mod simpl_03_constant_fold;
mod simpl_04_beta_contraction;
mod simpl_05_eta_reduction;

use std::collections::HashMap;

use crate::ir::cps::{Expr, Fun, Cont, Val};

// ── Infrastructure ──────────────────────────────────────────────────────────

pub(crate) type Census = HashMap<String, usize>;

pub(crate) fn census_name(census: &mut Census, name: &str) {
    *census.entry(name.to_string()).or_insert(0) += 1;
}

pub(crate) fn census_val(census: &mut Census, val: &Val) {
    match val {
        Val::Var(n) => census_name(census, n),
        Val::Cont(cont) => census_cont(census, cont),
        Val::Ctor(_, fields) => {
            for f in fields {
                census_name(census, f);
            }
        }
        Val::Field(n, _) => census_name(census, n),
        Val::Int(_) => {}
        Val::Prim(_, args) => {
            for a in args {
                census_name(census, a);
            }
        }
    }
}

pub(crate) fn census_fun(census: &mut Census, fun: &Fun) {
    census_expr(census, &fun.body);
}

pub(crate) fn census_cont(census: &mut Census, cont: &Cont) {
    census_expr(census, &cont.body);
}

pub(crate) fn census_expr(census: &mut Census, expr: &Expr) {
    match expr {
        Expr::Let(_, val, body) => {
            census_val(census, val);
            census_expr(census, body);
        }
        Expr::Letrec(_, fun, body) => {
            census_fun(census, fun);
            census_expr(census, body);
        }
        Expr::Encore(f, x, k) => {
            census_name(census, f);
            census_name(census, x);
            census_name(census, k);
        }
        Expr::Return(k, x) => {
            census_name(census, k);
            census_name(census, x);
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

pub(crate) fn count(census: &Census, name: &str) -> usize {
    census.get(name).copied().unwrap_or(0)
}

pub(crate) fn is_pure(val: &Val) -> bool {
    match val {
        Val::Var(_) | Val::Int(_) | Val::Cont(_) => true,
        Val::Ctor(_, _) | Val::Field(_, _) | Val::Prim(_, _) => true,
    }
}

pub(crate) use crate::pass::subst::subst_expr;

// ── Re-exports ──────────────────────────────────────────────────────────────

pub use simpl_01_dead_code::dead_code;
pub use simpl_02_copy_propagation::copy_propagation;
pub use simpl_03_constant_fold::constant_fold;
pub use simpl_04_beta_contraction::beta_contraction;
pub use simpl_05_eta_reduction::eta_reduction;
