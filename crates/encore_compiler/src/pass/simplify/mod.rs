mod simpl_01_dead_code;
mod simpl_02_copy_propagation;
mod simpl_03_constant_fold;
mod simpl_04_beta_contraction;
mod simpl_05_eta_reduction;

use std::collections::HashMap;

use crate::ir::cps::{Expr, Lambda, Val};

// ── Infrastructure ──────────────────────────────────────────────────────────

pub(crate) type Census = HashMap<String, usize>;

pub(crate) fn census_name(census: &mut Census, name: &str) {
    *census.entry(name.to_string()).or_insert(0) += 1;
}

pub(crate) fn census_val(census: &mut Census, val: &Val) {
    match val {
        Val::Var(n) => census_name(census, n),
        Val::Lambda(lam) => census_lambda(census, lam),
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

pub(crate) fn census_lambda(census: &mut Census, lam: &Lambda) {
    census_expr(census, &lam.body);
}

pub(crate) fn census_expr(census: &mut Census, expr: &Expr) {
    match expr {
        Expr::Let(_, val, body) => {
            census_val(census, val);
            census_expr(census, body);
        }
        Expr::Letrec(_, lam, body) => {
            census_lambda(census, lam);
            census_expr(census, body);
        }
        Expr::App(f, x) => {
            census_name(census, f);
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
        Val::Var(_) | Val::Int(_) | Val::Lambda(_) => true,
        Val::Ctor(_, _) | Val::Field(_, _) | Val::Prim(_, _) => true,
    }
}

pub(crate) fn subst_name(from: &str, to: &str, name: &mut String) {
    if name == from {
        *name = to.to_string();
    }
}

pub(crate) fn subst_val(from: &str, to: &str, val: &mut Val) {
    match val {
        Val::Var(n) => subst_name(from, to, n),
        Val::Lambda(lam) => {
            if lam.param != from {
                subst_expr(from, to, &mut lam.body);
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
    }
}

pub(crate) fn subst_expr(from: &str, to: &str, expr: &mut Expr) {
    match expr {
        Expr::Let(binder, val, body) => {
            subst_val(from, to, val);
            if binder != from {
                subst_expr(from, to, body);
            }
        }
        Expr::Letrec(binder, lam, body) => {
            if binder != from {
                if lam.param != from {
                    subst_expr(from, to, &mut lam.body);
                }
                subst_expr(from, to, body);
            }
        }
        Expr::App(f, x) => {
            subst_name(from, to, f);
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

// ── Re-exports ──────────────────────────────────────────────────────────────

pub use simpl_01_dead_code::dead_code;
pub use simpl_02_copy_propagation::copy_propagation;
pub use simpl_03_constant_fold::constant_fold;
pub use simpl_04_beta_contraction::beta_contraction;
pub use simpl_05_eta_reduction::eta_reduction;
