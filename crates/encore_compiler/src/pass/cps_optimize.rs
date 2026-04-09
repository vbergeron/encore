use crate::ir::cps::{self, Expr};

use super::rewrite;
use super::simplify;

// ── Configuration ───────────────────────────────────────────────────────────

pub struct OptimizeConfig {
    pub fuel: usize,
    pub inline_threshold: usize,
}

impl Default for OptimizeConfig {
    fn default() -> Self {
        Self {
            fuel: 100,
            inline_threshold: 20,
        }
    }
}

// ── Simplifier ──────────────────────────────────────────────────────────────

fn simplify(mut expr: Expr, fuel: &mut usize) -> Expr {
    loop {
        if *fuel == 0 {
            break;
        }
        let before = expr.clone();
        expr = simplify::dead_code(expr);
        expr = simplify::copy_propagation(expr);
        expr = simplify::constant_fold(expr);
        expr = simplify::beta_contraction(expr);
        expr = simplify::eta_reduction(expr);

        if expr == before {
            break;
        }
        *fuel -= 1;
    }
    expr
}

// ── Optimizer loop ──────────────────────────────────────────────────────────

fn optimize_expr(expr: Expr, config: &mut OptimizeConfig) -> Expr {
    let mut expr = expr;
    loop {
        if config.fuel == 0 {
            break;
        }
        let before = expr.clone();

        expr = simplify(expr, &mut config.fuel);

        expr = rewrite::inlining(expr, config.inline_threshold);
        expr = simplify(expr, &mut config.fuel);

        expr = rewrite::hoisting(expr);
        expr = simplify(expr, &mut config.fuel);

        expr = rewrite::cse(expr);
        expr = simplify(expr, &mut config.fuel);

        if expr == before {
            break;
        }
        config.fuel = config.fuel.saturating_sub(1);
    }
    expr
}

pub fn optimize_module(module: cps::Module, config: OptimizeConfig) -> cps::Module {
    cps::Module {
        defines: module
            .defines
            .into_iter()
            .map(|d| {
                let mut cfg = OptimizeConfig {
                    fuel: config.fuel,
                    inline_threshold: config.inline_threshold,
                };
                cps::Define {
                    name: d.name,
                    body: optimize_expr(d.body, &mut cfg),
                }
            })
            .collect(),
    }
}
