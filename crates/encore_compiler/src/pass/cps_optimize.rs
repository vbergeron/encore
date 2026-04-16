use crate::ir::cps::{self, Expr, Fun};

use super::cps_rewrite;
use super::cps_rewrite::GlobalFuns;
use super::cps_simplify;

// ── Configuration ───────────────────────────────────────────────────────────

pub struct OptimizeConfig {
    pub fuel: usize,
    pub inline_threshold: usize,
    pub verbose: bool,

    pub simplify_dead_code: bool,
    pub simplify_copy_propagation: bool,
    pub simplify_constant_fold: bool,
    pub simplify_beta_contraction: bool,
    pub simplify_eta_reduction: bool,

    pub rewrite_inlining: bool,
    pub rewrite_hoisting: bool,
    pub rewrite_cse: bool,
    pub rewrite_contification: bool,
}

impl Default for OptimizeConfig {
    fn default() -> Self {
        Self {
            fuel: 100,
            inline_threshold: 8,
            verbose: false,

            simplify_dead_code: true,
            simplify_copy_propagation: true,
            simplify_constant_fold: true,
            simplify_beta_contraction: true,
            simplify_eta_reduction: true,

            rewrite_inlining: true,
            rewrite_hoisting: true,
            rewrite_cse: true,
            rewrite_contification: true,
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
            expr = cps_simplify::dead_code(expr);
        }
        if config.simplify_copy_propagation {
            expr = cps_simplify::copy_propagation(expr);
        }
        if config.simplify_constant_fold {
            expr = cps_simplify::constant_fold(expr);
        }
        if config.simplify_beta_contraction {
            expr = cps_simplify::beta_contraction(expr);
        }
        if config.simplify_eta_reduction {
            expr = cps_simplify::eta_reduction(expr);
        }

        if expr == before {
            break;
        }
        *fuel -= 1;
    }
    expr
}

// ── Optimizer loop ──────────────────────────────────────────────────────────

fn optimize_expr(
    expr: Expr,
    config: &OptimizeConfig,
    fuel: &mut usize,
    globals: &GlobalFuns,
    define_name: &str,
) -> Expr {
    let trace = |label: &str, e: &Expr| {
        if config.verbose {
            eprintln!("--- {define_name}: after {label} ---\n{e}\n");
        }
    };

    // One-shot global inlining before the fixpoint loop
    let mut expr = if config.rewrite_inlining && !globals.is_empty() {
        let e = cps_rewrite::inlining(expr, config.inline_threshold, globals);
        trace("global_inlining", &e);
        let e = run_simplify(e, config, fuel);
        trace("initial_simplify", &e);
        e
    } else {
        expr
    };

    let mut iteration = 0;
    loop {
        if *fuel == 0 {
            break;
        }
        let before = expr.clone();
        iteration += 1;

        expr = run_simplify(expr, config, fuel);

        if config.rewrite_inlining {
            expr = cps_rewrite::inlining(expr, config.inline_threshold, &Default::default());
            trace(&format!("inlining (iter {iteration})"), &expr);
            expr = run_simplify(expr, config, fuel);
        }

        if config.rewrite_hoisting {
            expr = cps_rewrite::hoisting(expr);
            trace(&format!("hoisting (iter {iteration})"), &expr);
            expr = run_simplify(expr, config, fuel);
        }

        if config.rewrite_cse {
            expr = cps_rewrite::cse(expr);
            trace(&format!("cse (iter {iteration})"), &expr);
            expr = run_simplify(expr, config, fuel);
        }

        if config.rewrite_contification {
            expr = cps_rewrite::contification(expr);
            trace(&format!("contification (iter {iteration})"), &expr);
            expr = run_simplify(expr, config, fuel);
        }

        if expr == before {
            break;
        }
        *fuel = fuel.saturating_sub(1);
    }
    expr
}

fn extract_fun(body: &Expr) -> Option<(&str, &Fun)> {
    match body {
        Expr::Letrec(name, fun, rest) => match rest.as_ref() {
            Expr::Fin(n) if n == name => Some((name, fun)),
            _ => None,
        },
        _ => None,
    }
}

fn mentions(name: &str, expr: &Expr) -> bool {
    match expr {
        Expr::Let(_, val, body) => mentions_val(name, val) || mentions(name, body),
        Expr::Letrec(n, fun, body) => {
            (n != name && mentions(name, &fun.body)) || mentions(name, body)
        }
        Expr::Encore(f, args, k) => {
            f == name || args.iter().any(|a| a == name) || k == name
        }
        Expr::Match(n, _, cases) => {
            n == name || cases.iter().any(|c| mentions(name, &c.body))
        }
        Expr::Fin(n) => n == name,
    }
}

fn mentions_val(name: &str, val: &cps::Val) -> bool {
    match val {
        cps::Val::Cont(cont) => mentions(name, &cont.body),
        cps::Val::Var(n) => n == name,
        cps::Val::Ctor(_, fields) => fields.iter().any(|f| f == name),
        cps::Val::Field(n, _) => n == name,
        cps::Val::Prim(_, args) => args.iter().any(|a| a == name),
        _ => false,
    }
}

fn collect_global_funs(defines: &[cps::Define], threshold: usize) -> GlobalFuns {
    let mut globals = GlobalFuns::new();
    for d in defines {
        if let Some((inner_name, fun)) = extract_fun(&d.body) {
            if !mentions(inner_name, &fun.body)
                && cps_rewrite::expr_size(&fun.body) <= threshold
            {
                globals.insert(d.name.clone(), fun.clone());
            }
        }
    }
    globals
}

fn optimize_define(
    define: cps::Define,
    config: &OptimizeConfig,
    globals: &GlobalFuns,
) -> cps::Define {
    if config.verbose {
        eprintln!("=== optimizing: {} ===\n{}\n", define.name, define.body);
    }
    let mut fuel = config.fuel;
    let body = optimize_expr(define.body, config, &mut fuel, globals, &define.name);
    if config.verbose {
        eprintln!("=== result: {} ===\n{}\n", define.name, body);
    }
    cps::Define { name: define.name, body }
}

pub fn optimize_module(module: cps::Module, config: OptimizeConfig) -> cps::Module {
    let globals = collect_global_funs(&module.defines, config.inline_threshold);
    cps::Module {
        defines: module
            .defines
            .into_iter()
            .map(|d| optimize_define(d, &config, &globals))
            .collect(),
    }
}
