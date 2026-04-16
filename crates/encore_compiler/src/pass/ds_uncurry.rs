use std::collections::HashSet;
use crate::ir::ds;

pub fn resolve_module(module: ds::Module) -> ds::Module {
    let escaping = find_escaping_globals(&module);
    let globals: Vec<(String, Option<usize>)> = module
        .defines
        .iter()
        .map(|d| {
            let arity = if escaping.contains(&d.name) {
                expr_arity(&d.body)
            } else {
                deep_arity(&d.body)
            };
            (d.name.clone(), arity)
        })
        .collect();
    ds::Module {
        defines: module
            .defines
            .into_iter()
            .map(|d| {
                let body = if escaping.contains(&d.name) {
                    d.body
                } else {
                    uncurry_expr(d.body)
                };
                ds::Define {
                    name: d.name,
                    body: resolve(&globals, body),
                }
            })
            .collect(),
    }
}

fn expr_arity(expr: &ds::Expr) -> Option<usize> {
    match expr {
        ds::Expr::Lambda(params, _) => Some(params.len()),
        _ => None,
    }
}

fn deep_arity(expr: &ds::Expr) -> Option<usize> {
    match expr {
        ds::Expr::Lambda(params, body) => Some(params.len() + nested_lambda_arity(body)),
        _ => None,
    }
}

fn nested_lambda_arity(expr: &ds::Expr) -> usize {
    match expr {
        ds::Expr::Lambda(params, body) => params.len() + nested_lambda_arity(body),
        _ => 0,
    }
}

fn find_escaping_globals(module: &ds::Module) -> HashSet<String> {
    let global_names: HashSet<&str> = module.defines.iter().map(|d| d.name.as_str()).collect();
    let mut escaping = HashSet::new();
    for d in &module.defines {
        collect_escaping(&global_names, &d.body, &mut escaping);
    }
    escaping
}

fn collect_escaping(globals: &HashSet<&str>, expr: &ds::Expr, escaping: &mut HashSet<String>) {
    match expr {
        ds::Expr::Apply(f, args) => {
            for a in args {
                collect_escaping_val(globals, a, escaping);
            }
            match f.as_ref() {
                ds::Expr::Var(name) if globals.contains(name.as_str()) => {}
                _ => collect_escaping(globals, f, escaping),
            }
        }
        ds::Expr::Var(name) if globals.contains(name.as_str()) => {
            escaping.insert(name.clone());
        }
        ds::Expr::Lambda(_, body) => collect_escaping(globals, body, escaping),
        ds::Expr::Let(_, bound, body) => {
            collect_escaping(globals, bound, escaping);
            collect_escaping(globals, body, escaping);
        }
        ds::Expr::Letrec(_, _, fun_body, rest) => {
            collect_escaping(globals, fun_body, escaping);
            collect_escaping(globals, rest, escaping);
        }
        ds::Expr::Ctor(_, fields) => {
            for f in fields { collect_escaping_val(globals, f, escaping); }
        }
        ds::Expr::Field(e, _) => collect_escaping(globals, e, escaping),
        ds::Expr::Match(scrut, _, cases) => {
            collect_escaping(globals, scrut, escaping);
            for c in cases { collect_escaping(globals, &c.body, escaping); }
        }
        ds::Expr::Prim(_, args) => {
            for a in args { collect_escaping_val(globals, a, escaping); }
        }
        _ => {}
    }
}

fn collect_escaping_val(globals: &HashSet<&str>, expr: &ds::Expr, escaping: &mut HashSet<String>) {
    match expr {
        ds::Expr::Var(name) if globals.contains(name.as_str()) => {
            escaping.insert(name.clone());
        }
        _ => collect_escaping(globals, expr, escaping),
    }
}

struct FreshGen {
    counter: usize,
}

impl FreshGen {
    fn new() -> Self {
        Self { counter: 0 }
    }

    fn fresh(&mut self) -> String {
        let n = self.counter;
        self.counter += 1;
        format!("__pa{n}")
    }
}

fn resolve(env: &[(String, Option<usize>)], expr: ds::Expr) -> ds::Expr {
    match expr {
        ds::Expr::Var(_) | ds::Expr::Int(_) | ds::Expr::Bytes(_) | ds::Expr::Extern(_) => expr,

        ds::Expr::Lambda(params, body) => {
            let mut e = env.to_vec();
            for p in &params {
                e.push((p.clone(), None));
            }
            ds::Expr::Lambda(params, Box::new(resolve(&e, *body)))
        }

        ds::Expr::Apply(f, args) => {
            let f = resolve(env, *f);
            let args: Vec<ds::Expr> = args.into_iter().map(|a| resolve(env, a)).collect();
            rewrite_apply(&mut FreshGen::new(), env, f, args)
        }

        ds::Expr::Let(name, bound, body) => {
            let bound = resolve(env, *bound);
            let arity = expr_arity(&bound);
            let mut e = env.to_vec();
            e.push((name.clone(), arity));
            ds::Expr::Let(name, Box::new(bound), Box::new(resolve(&e, *body)))
        }

        ds::Expr::Letrec(fname, param, fun_body, rest) => {
            let mut env_f = env.to_vec();
            env_f.push((fname.clone(), Some(1)));
            let mut env_fp = env_f.clone();
            env_fp.push((param.clone(), None));
            ds::Expr::Letrec(
                fname,
                param,
                Box::new(resolve(&env_fp, *fun_body)),
                Box::new(resolve(&env_f, *rest)),
            )
        }

        ds::Expr::Ctor(tag, fields) => {
            ds::Expr::Ctor(tag, fields.into_iter().map(|f| resolve(env, f)).collect())
        }

        ds::Expr::Field(e, idx) => ds::Expr::Field(Box::new(resolve(env, *e)), idx),

        ds::Expr::Match(scrutinee, base, cases) => {
            let scrutinee = resolve(env, *scrutinee);
            let cases = cases
                .into_iter()
                .map(|c| {
                    let mut e = env.to_vec();
                    for b in &c.binds {
                        e.push((b.clone(), None));
                    }
                    ds::Case {
                        binds: c.binds,
                        body: resolve(&e, c.body),
                    }
                })
                .collect();
            ds::Expr::Match(Box::new(scrutinee), base, cases)
        }

        ds::Expr::Prim(op, args) => {
            ds::Expr::Prim(op, args.into_iter().map(|a| resolve(env, a)).collect())
        }
    }
}

fn uncurry_expr(expr: ds::Expr) -> ds::Expr {
    match expr {
        ds::Expr::Lambda(params, body) => {
            let mut all_params = params;
            let mut inner = *body;
            while let ds::Expr::Lambda(p, b) = inner {
                all_params.extend(p);
                inner = *b;
            }
            ds::Expr::Lambda(all_params, Box::new(inner))
        }
        other => other,
    }
}

fn lookup_arity(env: &[(String, Option<usize>)], name: &str) -> Option<usize> {
    env.iter().rev().find(|(n, _)| n == name).and_then(|(_, a)| *a)
}

fn rewrite_apply(fg: &mut FreshGen, env: &[(String, Option<usize>)], head: ds::Expr, args: Vec<ds::Expr>) -> ds::Expr {
    if args.is_empty() {
        return head;
    }

    let known_arity = match &head {
        ds::Expr::Var(name) => lookup_arity(env, name),
        ds::Expr::Lambda(params, _) => Some(params.len()),
        _ => None,
    };

    match known_arity {
        Some(n) => {
            let n_args = args.len();
            match n_args.cmp(&n) {
                std::cmp::Ordering::Equal => {
                    ds::Expr::Apply(Box::new(head), args)
                }
                std::cmp::Ordering::Less => {
                    let remaining = n - n_args;
                    let fresh_params: Vec<String> = (0..remaining).map(|_| fg.fresh()).collect();
                    let mut full_args = args;
                    for p in &fresh_params {
                        full_args.push(ds::Expr::Var(p.clone()));
                    }
                    let inner = ds::Expr::Apply(Box::new(head), full_args);
                    fresh_params.into_iter().rev().fold(inner, |body, p| {
                        ds::Expr::Lambda(vec![p], Box::new(body))
                    })
                }
                std::cmp::Ordering::Greater => {
                    let (exact_args, rest_args) = split_vec(args, n);
                    let inner = ds::Expr::Apply(Box::new(head), exact_args);
                    curry_unknown(inner, rest_args)
                }
            }
        }
        None => {
            curry_unknown(head, args)
        }
    }
}

fn curry_unknown(head: ds::Expr, args: Vec<ds::Expr>) -> ds::Expr {
    args.into_iter().fold(head, |acc, arg| {
        ds::Expr::Apply(Box::new(acc), vec![arg])
    })
}

fn split_vec<T>(mut v: Vec<T>, at: usize) -> (Vec<T>, Vec<T>) {
    let rest = v.split_off(at);
    (v, rest)
}
