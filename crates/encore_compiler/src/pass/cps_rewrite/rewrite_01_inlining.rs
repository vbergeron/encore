use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::ir::cps::{self, Expr, Fun, Cont, Val};
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
    inline_expr(expr, threshold, &HashMap::new(), globals)
}

type Env = HashMap<String, Cont>;

pub fn expr_size(expr: &Expr) -> usize {
    match expr {
        Expr::Let(_, val, body) => 1 + val_size(val) + expr_size(body),
        Expr::Letrec(_, fun, body) => 1 + expr_size(&fun.body) + expr_size(body),
        Expr::Encore(_, _, _) => 1,
        Expr::Match(_, _, cases) => {
            1 + cases.iter().map(|c| expr_size(&c.body)).sum::<usize>()
        }
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
    let renamed_params: Vec<String> = fun.args.iter().map(|p| {
        let fresh = format!("{}{}", p, fresh_suffix());
        renames.insert(p.clone(), fresh.clone());
        fresh
    }).collect();
    let renamed_cont = format!("{}{}", fun.cont, fresh_suffix());
    renames.insert(fun.cont.clone(), renamed_cont.clone());
    alpha_rename_expr(&mut body, &mut renames);
    for (renamed_param, arg) in renamed_params.iter().zip(args.iter()) {
        subst_expr(renamed_param, arg, &mut body);
    }
    subst_expr(&renamed_cont, k, &mut body);
    body
}

fn inline_expr(expr: Expr, threshold: usize, env: &Env, globals: &GlobalFuns) -> Expr {
    match expr {
        Expr::Let(name, Val::Cont(cont), body) => {
            let cont = Cont {
                params: cont.params,
                body: Box::new(inline_expr(*cont.body, threshold, env, globals)),
            };
            let mut env = env.clone();
            if expr_size(&cont.body) <= threshold {
                env.insert(name.clone(), cont.clone());
            }
            let body = inline_expr(*body, threshold, &env, globals);
            Expr::Let(name, Val::Cont(cont), Box::new(body))
        }
        Expr::Let(name, val, body) => {
            let val = inline_val(val, threshold, env, globals);
            let body = inline_expr(*body, threshold, env, globals);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Letrec(name, fun, body) => {
            let fun = Fun {
                args: fun.args,
                cont: fun.cont,
                body: Box::new(inline_expr(*fun.body, threshold, env, globals)),
            };
            let body = inline_expr(*body, threshold, env, globals);
            Expr::Letrec(name, fun, Box::new(body))
        }
        Expr::Encore(ref f, ref args, ref k) => {
            if let Some(cont) = env.get(f) {
                if args.len() == cont.params.len() {
                    let mut body = *cont.body.clone();
                    for (param, arg) in cont.params.iter().zip(args.iter()) {
                        subst_expr(param, arg, &mut body);
                    }
                    return body;
                }
            }
            if let Some(fun) = globals.get(f) {
                if fun.args.len() == args.len() {
                    return inline_global(fun, args, k);
                }
            }
            expr
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case {
                    binds: c.binds,
                    body: inline_expr(c.body, threshold, env, globals),
                })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn inline_val(val: Val, threshold: usize, env: &Env, globals: &GlobalFuns) -> Val {
    match val {
        Val::Cont(cont) => Val::Cont(Cont {
            params: cont.params,
            body: Box::new(inline_expr(*cont.body, threshold, env, globals)),
        }),
        other => other,
    }
}
