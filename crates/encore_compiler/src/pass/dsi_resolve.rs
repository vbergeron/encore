use crate::ir::{ds, dsi};

pub fn resolve_module(module: ds::Module) -> dsi::Module {
    let globals: Vec<String> = module.defines.iter().map(|d| d.name.clone()).collect();
    dsi::Module {
        defines: module
            .defines
            .into_iter()
            .map(|d| dsi::Define {
                name: d.name,
                body: resolve(&globals, d.body),
            })
            .collect(),
    }
}

fn resolve(env: &[String], expr: ds::Expr) -> dsi::Expr {
    match expr {
        ds::Expr::Var(name) => {
            let idx = env
                .iter()
                .rev()
                .position(|n| n == &name)
                .unwrap_or_else(|| panic!("unbound variable: {name}"));
            dsi::Expr::Var(idx)
        }

        ds::Expr::Lam(x, body) => {
            let mut e = env.to_vec();
            e.push(x);
            dsi::Expr::Lam(Box::new(resolve(&e, *body)))
        }

        ds::Expr::LamN(params, body) => {
            let mut e = env.to_vec();
            let n = params.len();
            for p in params {
                e.push(p);
            }
            dsi::Expr::LamN(n, Box::new(resolve(&e, *body)))
        }

        ds::Expr::App(e1, e2) => dsi::Expr::App(
            Box::new(resolve(env, *e1)),
            Box::new(resolve(env, *e2)),
        ),

        ds::Expr::AppN(f, args) => dsi::Expr::AppN(
            Box::new(resolve(env, *f)),
            args.into_iter().map(|a| resolve(env, a)).collect(),
        ),

        ds::Expr::Let(x, bound, body) => {
            let bound = resolve(env, *bound);
            let mut e = env.to_vec();
            e.push(x);
            dsi::Expr::Let(Box::new(bound), Box::new(resolve(&e, *body)))
        }

        ds::Expr::Letrec(f, x, fun_body, rest) => {
            let mut env_f = env.to_vec();
            env_f.push(f);
            let mut env_fx = env_f.clone();
            env_fx.push(x);
            dsi::Expr::Letrec(
                Box::new(resolve(&env_fx, *fun_body)),
                Box::new(resolve(&env_f, *rest)),
            )
        }

        ds::Expr::Ctor(tag, fields) => {
            dsi::Expr::Ctor(tag, fields.into_iter().map(|f| resolve(env, f)).collect())
        }

        ds::Expr::Field(e, idx) => dsi::Expr::Field(Box::new(resolve(env, *e)), idx),

        ds::Expr::Match(scrutinee, base, cases) => {
            let scrutinee = resolve(env, *scrutinee);
            let cases = cases
                .into_iter()
                .map(|c| {
                    let mut e = env.to_vec();
                    for b in &c.binds {
                        e.push(b.clone());
                    }
                    dsi::Case {
                        arity: c.binds.len(),
                        body: resolve(&e, c.body),
                    }
                })
                .collect();
            dsi::Expr::Match(Box::new(scrutinee), base, cases)
        }

        ds::Expr::Int(n) => dsi::Expr::Int(n),

        ds::Expr::Prim(op, args) => {
            dsi::Expr::Prim(op, args.into_iter().map(|a| resolve(env, a)).collect())
        }

        ds::Expr::Extern(slot) => dsi::Expr::Extern(slot),
    }
}
