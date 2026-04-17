use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ir::cps::{Cont, Expr, Fun, Val};
use crate::ir::cps_traversal::CPSTransformer;
use crate::pass::cps_subst::subst_expr;

pub type GlobalFuns = HashMap<String, Fun>;

static INLINE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn fresh_suffix() -> String {
    let n = INLINE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("_i{n}")
}

fn alpha_rename_expr(expr: &mut Expr, renames: &mut HashMap<String, String>) {
    match expr {
        Expr::Let(binder, val, body) => {
            alpha_rename_val(val, renames);
            let fresh = format!("{}{}", binder, fresh_suffix());
            renames.insert(binder.clone(), fresh.clone());
            *binder = fresh;
            alpha_rename_expr(body, renames);
        }
        Expr::Letrec(binder, fun, body) => {
            let fresh = format!("{}{}", binder, fresh_suffix());
            renames.insert(binder.clone(), fresh.clone());
            *binder = fresh;
            for arg in &mut fun.args {
                let fa = format!("{}{}", arg, fresh_suffix());
                renames.insert(arg.clone(), fa.clone());
                *arg = fa;
            }
            let fc = format!("{}{}", fun.cont, fresh_suffix());
            renames.insert(fun.cont.clone(), fc.clone());
            fun.cont = fc;
            alpha_rename_expr(&mut fun.body, renames);
            alpha_rename_expr(body, renames);
        }
        Expr::Encore(f, args, k) => {
            rename_name(f, renames);
            for a in args {
                rename_name(a, renames);
            }
            rename_name(k, renames);
        }
        Expr::Match(n, _, cases) => {
            rename_name(n, renames);
            for case in cases {
                for b in &mut case.binds {
                    let fb = format!("{}{}", b, fresh_suffix());
                    renames.insert(b.clone(), fb.clone());
                    *b = fb;
                }
                alpha_rename_expr(&mut case.body, renames);
            }
        }
        Expr::Fin(n) => {
            rename_name(n, renames);
        }
    }
}

fn alpha_rename_val(val: &mut Val, renames: &mut HashMap<String, String>) {
    match val {
        Val::Var(n) => rename_name(n, renames),
        Val::Cont(cont) => {
            for p in &mut cont.params {
                let fp = format!("{}{}", p, fresh_suffix());
                renames.insert(p.clone(), fp.clone());
                *p = fp;
            }
            alpha_rename_expr(&mut cont.body, renames);
        }
        Val::Ctor(_, fields) => {
            for f in fields {
                rename_name(f, renames);
            }
        }
        Val::Field(n, _) => rename_name(n, renames),
        Val::Prim(_, args) => {
            for a in args {
                rename_name(a, renames);
            }
        }
        Val::Int(_) | Val::Bytes(_) | Val::NullCont | Val::Extern(_) => {}
    }
}

fn rename_name(name: &mut String, renames: &HashMap<String, String>) {
    if let Some(new) = renames.get(name.as_str()) {
        *name = new.clone();
    }
}

pub fn inlining(expr: Expr, threshold: usize, globals: &GlobalFuns) -> Expr {
    Inlining { threshold, globals }.transform_expr(&mut Env::new(), expr)
}

type Env = HashMap<String, Cont>;

struct Inlining<'g> {
    threshold: usize,
    globals: &'g GlobalFuns,
}

impl<'g> CPSTransformer for Inlining<'g> {
    type Ctx = Env;

    fn transform_let(&self, env: &mut Env, name: String, val: Val, body: Expr) -> Expr {
        if let Val::Cont(cont) = val {
            let cont = self.transform_cont(env, cont);
            let mut local = env.clone();
            if expr_size(&cont.body) <= self.threshold {
                local.insert(name.clone(), cont.clone());
            }
            let body = self.transform_expr(&mut local, body);
            Expr::Let(name, Val::Cont(cont), Box::new(body))
        } else {
            Expr::Let(
                name,
                self.transform_val(env, val),
                Box::new(self.transform_expr(env, body)),
            )
        }
    }

    fn transform_encore(
        &self,
        env: &mut Env,
        f: String,
        args: Vec<String>,
        k: String,
    ) -> Expr {
        if let Some(cont) = env.get(&f) {
            if args.len() == cont.params.len() {
                let mut body = *cont.body.clone();
                for (param, arg) in cont.params.iter().zip(args.iter()) {
                    subst_expr(param, arg, &mut body);
                }
                return body;
            }
        }
        if let Some(fun) = self.globals.get(&f) {
            if fun.args.len() == args.len() {
                return inline_global(fun, &args, &k);
            }
        }
        Expr::Encore(f, args, k)
    }
}

pub fn expr_size(expr: &Expr) -> usize {
    match expr {
        Expr::Let(_, val, body) => 1 + val_size(val) + expr_size(body),
        Expr::Letrec(_, fun, body) => 1 + expr_size(&fun.body) + expr_size(body),
        Expr::Encore(_, _, _) => 1,
        Expr::Match(_, _, cases) => 1 + cases.iter().map(|c| expr_size(&c.body)).sum::<usize>(),
        Expr::Fin(_) => 1,
    }
}

fn val_size(val: &Val) -> usize {
    match val {
        Val::Cont(cont) => expr_size(&cont.body),
        _ => 1,
    }
}

fn inline_global(fun: &Fun, args: &[String], k: &str) -> Expr {
    let mut body = *fun.body.clone();
    let mut renames = HashMap::new();
    // Alpha-rename function params and cont to globally fresh names
    // BEFORE calling alpha_rename_expr.  This avoids sequential-
    // substitution capture: without it, `subst(p1→a1)` can introduce
    // a name that a later `subst(p2→a2)` silently rewrites.
    let renamed_params: Vec<String> = fun
        .args
        .iter()
        .map(|p| {
            let fresh = format!("{}{}", p, fresh_suffix());
            renames.insert(p.clone(), fresh.clone());
            fresh
        })
        .collect();
    let renamed_cont = format!("{}{}", fun.cont, fresh_suffix());
    renames.insert(fun.cont.clone(), renamed_cont.clone());
    alpha_rename_expr(&mut body, &mut renames);
    for (renamed_param, arg) in renamed_params.iter().zip(args.iter()) {
        subst_expr(renamed_param, arg, &mut body);
    }
    subst_expr(&renamed_cont, k, &mut body);
    body
}
