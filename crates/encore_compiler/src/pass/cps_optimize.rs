use crate::ir::cps::{self, Expr};

use super::rewrite;
use super::simplify;

// ── Configuration ───────────────────────────────────────────────────────────

pub struct OptimizeConfig {
    pub fuel: usize,
    pub inline_threshold: usize,

    pub simplify_dead_code: bool,
    pub simplify_copy_propagation: bool,
    pub simplify_constant_fold: bool,
    pub simplify_beta_contraction: bool,
    pub simplify_eta_reduction: bool,

    pub rewrite_inlining: bool,
    pub rewrite_hoisting: bool,
    pub rewrite_cse: bool,
}

impl Default for OptimizeConfig {
    fn default() -> Self {
        Self {
            fuel: 100,
            inline_threshold: 20,

            simplify_dead_code: true,
            simplify_copy_propagation: true,
            simplify_constant_fold: true,
            simplify_beta_contraction: true,
            simplify_eta_reduction: true,

            rewrite_inlining: true,
            rewrite_hoisting: true,
            rewrite_cse: true,
        }
    }
}

// ── Simplifier ──────────────────────────────────────────────────────────────

fn run_simplify(mut expr: Expr, config: &OptimizeConfig, fuel: &mut usize) -> Expr {
    loop {
        if *fuel == 0 {
            break;
        }
        let before = expr.clone();

        if config.simplify_dead_code {
            expr = simplify::dead_code(expr);
        }
        if config.simplify_copy_propagation {
            expr = simplify::copy_propagation(expr);
        }
        if config.simplify_constant_fold {
            expr = simplify::constant_fold(expr);
        }
        if config.simplify_beta_contraction {
            expr = simplify::beta_contraction(expr);
        }
        if config.simplify_eta_reduction {
            expr = simplify::eta_reduction(expr);
        }

        if expr == before {
            break;
        }
        *fuel -= 1;
    }
    expr
}

// ── Optimizer loop ──────────────────────────────────────────────────────────

fn optimize_expr(expr: Expr, config: &OptimizeConfig, fuel: &mut usize) -> Expr {
    let mut expr = expr;
    loop {
        if *fuel == 0 {
            break;
        }
        let before = expr.clone();

        expr = run_simplify(expr, config, fuel);

        if config.rewrite_inlining {
            expr = rewrite::inlining(expr, config.inline_threshold);
            expr = run_simplify(expr, config, fuel);
        }

        if config.rewrite_hoisting {
            expr = rewrite::hoisting(expr);
            expr = run_simplify(expr, config, fuel);
        }

        if config.rewrite_cse {
            expr = rewrite::cse(expr);
            expr = run_simplify(expr, config, fuel);
        }

        if expr == before {
            break;
        }
        *fuel = fuel.saturating_sub(1);
    }
    expr
}

fn optimize_define(define: cps::Define, config: &OptimizeConfig) -> cps::Define {
    let mut fuel = config.fuel;
    cps::Define {
        name: define.name,
        body: optimize_expr(define.body, config, &mut fuel),
    }
}

pub fn optimize_module(module: cps::Module, config: OptimizeConfig) -> cps::Module {
    cps::Module {
        defines: module.defines.into_iter().map(|d| optimize_define(d, &config)).collect(),
    }
}
