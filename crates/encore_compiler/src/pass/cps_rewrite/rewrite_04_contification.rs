// Convert non-recursive Letrec(f, Fun, outer) into direct inlining (single-use)
// or Let(f, Cont, outer) (multi-use with a single known continuation).
//
// The CPS transform wraps every lambda in a Letrec, even non-recursive ones.
// Beta contraction and inlining only handle Let(Cont), so this pass bridges
// the gap by turning eligible Letrec into forms those passes can eliminate.
//
// Single-use:
//   letrec f = fun(x, k). B in <ctx>[encore f arg k0]
//   ──►  <ctx>[B[x := arg, k := k0]]
//
// Multi-use (same continuation in scope):
//   letrec f = fun(x, k). B in <ctx>[encore f a1 k0] ... [encore f a2 k0]
//   ──►  let f = cont(x). B[k := k0] in <ctx>[return f a1] ... [return f a2]

use crate::ir::cps::{self, Case, Cont, Expr, Fun, Val};
use crate::pass::cps_subst::subst_expr;
use crate::pass::cps_census::{census_expr, count, Census};

pub fn contification(expr: Expr) -> Expr {
    contify_expr(expr)
}

fn contify_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Letrec(name, fun, body) => {
            let fun = Fun {
                arg: fun.arg,
                cont: fun.cont,
                body: Box::new(contify_expr(*fun.body)),
            };
            let body = contify_expr(*body);

            if is_self_recursive(&name, &fun) {
                return Expr::Letrec(name, fun, Box::new(body));
            }

            let (calls, escapes) = classify_uses(&name, &body);

            if escapes {
                return Expr::Letrec(name, fun, Box::new(body));
            }

            if calls == 1 {
                return inline_call(&name, &fun, body);
            }

            if calls > 1 {
                if let Some(k0) = single_continuation(&name, &body) {
                    if !is_bound(&k0, &body) {
                        return contify_to_cont(name, fun, body, &k0);
                    }
                }
            }

            Expr::Letrec(name, fun, Box::new(body))
        }
        Expr::Let(name, val, body) => {
            let val = contify_val(val);
            let body = contify_expr(*body);
            Expr::Let(name, val, Box::new(body))
        }
        Expr::Match(name, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| Case {
                    binds: c.binds,
                    body: contify_expr(c.body),
                })
                .collect();
            Expr::Match(name, base, cases)
        }
        other => other,
    }
}

fn contify_val(val: Val) -> Val {
    match val {
        Val::Cont(cont) => Val::Cont(Cont {
            param: cont.param,
            body: Box::new(contify_expr(*cont.body)),
        }),
        other => other,
    }
}

fn is_self_recursive(name: &str, fun: &Fun) -> bool {
    if fun.arg == name || fun.cont == name {
        return false;
    }
    let mut census = Census::new();
    census_expr(&mut census, &fun.body);
    count(&census, name) > 0
}

// Classify how `name` is used in `expr`.
// Returns (call_count, escapes) where call_count is the number of
// Encore(name, _, _) sites and escapes is true if name appears in
// any other position (value, arg, cont, scrutinee, etc.).
fn classify_uses(name: &str, expr: &Expr) -> (usize, bool) {
    let mut calls = 0usize;
    let mut escapes = false;
    classify_expr(name, expr, &mut calls, &mut escapes);
    (calls, escapes)
}

fn classify_expr(name: &str, expr: &Expr, calls: &mut usize, escapes: &mut bool) {
    match expr {
        Expr::Let(binder, val, body) => {
            classify_val(name, val, calls, escapes);
            if binder != name {
                classify_expr(name, body, calls, escapes);
            }
        }
        Expr::Letrec(binder, fun, body) => {
            if binder != name {
                // Uses inside a nested function body count as escaping
                // (the function captures `name` as a free variable)
                if fun.arg != name && fun.cont != name {
                    let mut inner_calls = 0;
                    let mut inner_esc = false;
                    classify_expr(name, &fun.body, &mut inner_calls, &mut inner_esc);
                    if inner_calls > 0 || inner_esc {
                        *escapes = true;
                    }
                }
                classify_expr(name, body, calls, escapes);
            }
        }
        Expr::Encore(f, x, k) => {
            if f == name {
                *calls += 1;
            }
            if x == name || k == name {
                *escapes = true;
            }
        }
        Expr::Match(n, _, cases) => {
            if n == name {
                *escapes = true;
            }
            for c in cases {
                if !c.binds.contains(&name.to_string()) {
                    classify_expr(name, &c.body, calls, escapes);
                }
            }
        }
        Expr::Fin(n) => {
            if n == name {
                *escapes = true;
            }
        }
    }
}

fn classify_val(name: &str, val: &Val, calls: &mut usize, escapes: &mut bool) {
    match val {
        Val::Var(n) if n == name => *escapes = true,
        Val::Cont(cont) => {
            // Continuations execute in the same scope, so calls inside are
            // legitimate call sites, not escapes.
            if cont.param != name {
                classify_expr(name, &cont.body, calls, escapes);
            }
        }
        Val::Ctor(_, fields) => {
            if fields.iter().any(|f| f == name) {
                *escapes = true;
            }
        }
        Val::Field(n, _) if n == name => *escapes = true,
        Val::Prim(_, args) => {
            if args.iter().any(|a| a == name) {
                *escapes = true;
            }
        }
        _ => {}
    }
}

// Collect the continuation argument from all Encore(name, _, k) sites.
// Returns Some(k0) if every call uses the same continuation, None otherwise.
fn single_continuation(name: &str, expr: &Expr) -> Option<String> {
    let mut cont: Option<String> = None;
    if collect_conts(name, expr, &mut cont) {
        cont
    } else {
        None
    }
}

fn collect_conts(name: &str, expr: &Expr, cont: &mut Option<String>) -> bool {
    match expr {
        Expr::Let(binder, val, body) => {
            if let Val::Cont(c) = val {
                if c.param != name && !collect_conts(name, &c.body, cont) {
                    return false;
                }
            }
            if binder != name {
                collect_conts(name, body, cont)
            } else {
                true
            }
        }
        Expr::Letrec(binder, fun, body) => {
            if binder != name {
                if fun.arg != name && fun.cont != name {
                    if !collect_conts(name, &fun.body, cont) {
                        return false;
                    }
                }
                collect_conts(name, body, cont)
            } else {
                true
            }
        }
        Expr::Encore(f, _, k) => {
            if f == name {
                match cont {
                    None => {
                        *cont = Some(k.clone());
                        true
                    }
                    Some(k0) if k0 == k => true,
                    _ => false,
                }
            } else {
                true
            }
        }
        Expr::Match(_, _, cases) => {
            for c in cases {
                if !c.binds.contains(&name.to_string()) {
                    if !collect_conts(name, &c.body, cont) {
                        return false;
                    }
                }
            }
            true
        }
        _ => true,
    }
}

// Check if `target` is bound (by Let, Letrec, or Match case) inside `expr`.
fn is_bound(target: &str, expr: &Expr) -> bool {
    match expr {
        Expr::Let(binder, val, body) => {
            binder == target || is_bound_val(target, val) || is_bound(target, body)
        }
        Expr::Letrec(binder, fun, body) => {
            binder == target || is_bound(target, &fun.body) || is_bound(target, body)
        }
        Expr::Match(_, _, cases) => cases.iter().any(|c| {
            c.binds.iter().any(|b| b == target) || is_bound(target, &c.body)
        }),
        _ => false,
    }
}

fn is_bound_val(target: &str, val: &Val) -> bool {
    match val {
        Val::Cont(cont) => cont.param == target || is_bound(target, &cont.body),
        _ => false,
    }
}

// Single-use: walk outer and replace Encore(name, x, k) with fun.body[arg:=x, cont:=k].
fn inline_call(name: &str, fun: &Fun, expr: Expr) -> Expr {
    match expr {
        Expr::Encore(ref f, ref x, ref k) if f == name => {
            let mut body = *fun.body.clone();
            subst_expr(&fun.arg, x, &mut body);
            subst_expr(&fun.cont, k, &mut body);
            body
        }
        Expr::Let(n, val, body) => {
            let val = inline_call_val(name, fun, val);
            Expr::Let(n, val, Box::new(inline_call(name, fun, *body)))
        }
        Expr::Letrec(n, f, body) => {
            Expr::Letrec(n, f, Box::new(inline_call(name, fun, *body)))
        }
        Expr::Match(n, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case {
                    binds: c.binds,
                    body: inline_call(name, fun, c.body),
                })
                .collect();
            Expr::Match(n, base, cases)
        }
        other => other,
    }
}

fn inline_call_val(name: &str, fun: &Fun, val: Val) -> Val {
    match val {
        Val::Cont(cont) => Val::Cont(Cont {
            param: cont.param,
            body: Box::new(inline_call(name, fun, *cont.body)),
        }),
        other => other,
    }
}

// Multi-use: substitute cont param in body, rewrite Encore(name,x,k) → Let(_nc, NullCont, Encore(name,x,_nc)).
fn contify_to_cont(name: String, fun: Fun, outer: Expr, k0: &str) -> Expr {
    let mut body = *fun.body;
    subst_expr(&fun.cont, k0, &mut body);
    let cont = Val::Cont(Cont {
        param: fun.arg,
        body: Box::new(body),
    });
    let outer = rewrite_calls(&name, outer);
    Expr::Let(name, cont, Box::new(outer))
}

fn rewrite_calls(name: &str, expr: Expr) -> Expr {
    match expr {
        Expr::Encore(f, x, _) if f == name => {
            Expr::Let("_nc".into(), Val::NullCont, Box::new(Expr::Encore(f, x, "_nc".into())))
        }
        Expr::Let(n, val, body) => {
            let val = rewrite_calls_val(name, val);
            Expr::Let(n, val, Box::new(rewrite_calls(name, *body)))
        }
        Expr::Letrec(n, f, body) => {
            Expr::Letrec(n, f, Box::new(rewrite_calls(name, *body)))
        }
        Expr::Match(n, base, cases) => {
            let cases = cases
                .into_iter()
                .map(|c| cps::Case {
                    binds: c.binds,
                    body: rewrite_calls(name, c.body),
                })
                .collect();
            Expr::Match(n, base, cases)
        }
        other => other,
    }
}

fn rewrite_calls_val(name: &str, val: Val) -> Val {
    match val {
        Val::Cont(cont) => Val::Cont(Cont {
            param: cont.param,
            body: Box::new(rewrite_calls(name, *cont.body)),
        }),
        other => other,
    }
}
