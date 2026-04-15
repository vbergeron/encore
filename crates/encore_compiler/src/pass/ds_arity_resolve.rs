use crate::ir::ds;

pub fn resolve_module(module: ds::Module) -> ds::Module {
    let globals: Vec<(String, Option<usize>)> = module
        .defines
        .iter()
        .map(|d| (d.name.clone(), expr_arity(&d.body)))
        .collect();
    ds::Module {
        defines: module
            .defines
            .into_iter()
            .map(|d| ds::Define {
                name: d.name,
                body: resolve(&globals, d.body),
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

fn lookup_arity(env: &[(String, Option<usize>)], name: &str) -> Option<usize> {
    env.iter().rev().find(|(n, _)| n == name).and_then(|(_, a)| *a)
}

fn rewrite_apply(fg: &mut FreshGen, env: &[(String, Option<usize>)], head: ds::Expr, args: Vec<ds::Expr>) -> ds::Expr {
    if args.is_empty() {
        return head;
    }

    let known_arity = match &head {
        ds::Expr::Var(name) => lookup_arity(env, name),
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
                    ds::Expr::Lambda(
                        fresh_params,
                        Box::new(ds::Expr::Apply(Box::new(head), full_args)),
                    )
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
