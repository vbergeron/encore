use std::collections::HashMap;

use crate::ir::cps::{self, Expr, Lambda, Val};

type Census = HashMap<String, usize>;

fn census_name(census: &mut Census, name: &str) {
    *census.entry(name.to_string()).or_insert(0) += 1;
}

fn census_val(census: &mut Census, val: &Val) {
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

fn census_lambda(census: &mut Census, lam: &Lambda) {
    census_expr(census, &lam.body);
}

fn census_expr(census: &mut Census, expr: &Expr) {
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

fn count(census: &Census, name: &str) -> usize {
    census.get(name).copied().unwrap_or(0)
}

fn subst_name(from: &str, to: &str, name: &mut String) {
    if name == from {
        *name = to.to_string();
    }
}

fn subst_val(from: &str, to: &str, val: &mut Val) {
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

fn subst_expr(from: &str, to: &str, expr: &mut Expr) {
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

fn try_dead_let(name: &str, val: &Val, body: &Expr) -> bool {
    let mut census = Census::new();
    census_expr(&mut census, body);
    count(&census, name) == 0 && is_pure(val)
}

fn try_copy_propagation(name: &str, val: &Val, body: &mut Expr) -> bool {
    if let Val::Var(y) = val {
        subst_expr(name, y, body);
        true
    } else {
        false
    }
}

fn try_dead_letrec(name: &str, body: &Expr) -> bool {
    let mut census = Census::new();
    census_expr(&mut census, body);
    count(&census, name) == 0
}

fn contract_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Let(name, val, body) => {
            if try_dead_let(&name, &val, &body) {
                return contract_expr(*body);
            }

            let mut body = *body;
            if try_copy_propagation(&name, &val, &mut body) {
                return contract_expr(body);
            }

            let val = contract_val(val);
            let body = contract_expr(body);
            Expr::Let(name, val, Box::new(body))
        }

        Expr::Letrec(name, lam, body) => {
            if try_dead_letrec(&name, &body) {
                return contract_expr(*body);
            }

            let lam = contract_lambda(lam);
            let body = contract_expr(*body);
            Expr::Letrec(name, lam, Box::new(body))
        }

        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case {
                    binds: c.binds,
                    body: contract_expr(c.body),
                })
                .collect();
            Expr::Match(name, base, cases)
        }

        other => other,
    }
}

fn contract_val(val: Val) -> Val {
    match val {
        Val::Lambda(lam) => Val::Lambda(contract_lambda(lam)),
        other => other,
    }
}

fn contract_lambda(lam: Lambda) -> Lambda {
    Lambda {
        param: lam.param,
        body: Box::new(contract_expr(*lam.body)),
    }
}

fn is_pure(val: &Val) -> bool {
    match val {
        Val::Var(_) | Val::Int(_) | Val::Lambda(_) => true,
        Val::Ctor(_, _) | Val::Field(_, _) | Val::Prim(_, _) => true,
    }
}

pub fn optimize_expr(expr: Expr) -> Expr {
    let before = expr.clone();
    let after = contract_expr(expr);
    if after == before {
        after
    } else {
        optimize_expr(after)
    }
}

pub fn optimize_module(module: cps::Module) -> cps::Module {
    cps::Module {
        defines: module
            .defines
            .into_iter()
            .map(|d| cps::Define {
                name: d.name,
                body: optimize_expr(d.body),
            })
            .collect(),
    }
}
