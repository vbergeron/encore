use crate::ir::{cps, dsi, prim::PrimOp};

struct FreshGen {
    counter: usize,
}

impl FreshGen {
    fn new() -> Self {
        Self { counter: 0 }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        let n = self.counter;
        self.counter += 1;
        format!("_{prefix}{n}")
    }
}

type Cont = Box<dyn FnOnce(&mut FreshGen, String) -> cps::Expr>;

fn halt(_fg: &mut FreshGen, name: String) -> cps::Expr {
    cps::Expr::Fin(name)
}

pub fn transform_module(module: dsi::Module) -> cps::Module {
    let globals: Vec<String> = module.defines.iter().map(|d| d.name.clone()).collect();
    cps::Module {
        defines: module
            .defines
            .into_iter()
            .map(|d| {
                let mut fg = FreshGen::new();
                cps::Define {
                    name: d.name,
                    body: transform(&mut fg, &globals, d.body, Box::new(halt)),
                }
            })
            .collect(),
    }
}

fn transform(fg: &mut FreshGen, env: &[String], expr: dsi::Expr, k: Cont) -> cps::Expr {
    match expr {
        dsi::Expr::Var(idx) => {
            let name = env[env.len() - 1 - idx].clone();
            k(fg, name)
        }

        dsi::Expr::Lam(body) => {
            let f = fg.fresh("f");
            let x = fg.fresh("x");
            let kv = fg.fresh("k");
            let kv2 = kv.clone();
            let mut env_body = env.to_vec();
            env_body.push(x.clone());
            cps::Expr::Letrec(
                f.clone(),
                cps::Fun {
                    args: vec![x],
                    cont: kv,
                    body: Box::new(transform(fg, &env_body, *body, Box::new(move |fg, r| {
                        let nc = fg.fresh("nc");
                        cps::Expr::Let(nc.clone(), cps::Val::NullCont,
                            Box::new(cps::Expr::Encore(kv2, vec![r], nc)))
                    }))),
                },
                Box::new(k(fg, f)),
            )
        }

        dsi::Expr::LamN(n, body) => {
            let f = fg.fresh("f");
            let kv = fg.fresh("k");
            let kv2 = kv.clone();
            let mut env_body = env.to_vec();
            let mut args = Vec::with_capacity(n);
            for _ in 0..n {
                let x = fg.fresh("x");
                env_body.push(x.clone());
                args.push(x);
            }
            cps::Expr::Letrec(
                f.clone(),
                cps::Fun {
                    args,
                    cont: kv,
                    body: Box::new(transform(fg, &env_body, *body, Box::new(move |fg, r| {
                        let nc = fg.fresh("nc");
                        cps::Expr::Let(nc.clone(), cps::Val::NullCont,
                            Box::new(cps::Expr::Encore(kv2, vec![r], nc)))
                    }))),
                },
                Box::new(k(fg, f)),
            )
        }

        dsi::Expr::App(e1, e2) => {
            let env2 = env.to_vec();
            transform(fg, env, *e1, Box::new(move |fg, f| {
                transform(fg, &env2, *e2, Box::new(move |fg, x| {
                    let kn = fg.fresh("k");
                    let r = fg.fresh("r");
                    let r2 = r.clone();
                    cps::Expr::Let(
                        kn.clone(),
                        cps::Val::Cont(cps::Cont {
                            param: r,
                            body: Box::new(k(fg, r2)),
                        }),
                        Box::new(cps::Expr::Encore(f, vec![x], kn)),
                    )
                }))
            }))
        }

        dsi::Expr::AppN(ef, eargs) => {
            let env2 = env.to_vec();
            transform(fg, env, *ef, Box::new(move |fg, f| {
                transform_appn_args(fg, &env2, eargs, vec![], f, k)
            }))
        }

        dsi::Expr::Let(bound, body) => {
            let env_owned = env.to_vec();
            transform(fg, env, *bound, Box::new(move |fg, v| {
                let mut env_body = env_owned;
                env_body.push(v);
                transform(fg, &env_body, *body, k)
            }))
        }

        dsi::Expr::Letrec(fun_body, rest) => {
            let f = fg.fresh("f");
            let x = fg.fresh("x");
            let kv = fg.fresh("k");
            let kv2 = kv.clone();
            let mut env_fx = env.to_vec();
            env_fx.push(f.clone());
            env_fx.push(x.clone());
            let mut env_f = env.to_vec();
            env_f.push(f.clone());
            cps::Expr::Letrec(
                f,
                cps::Fun {
                    args: vec![x],
                    cont: kv,
                    body: Box::new(transform(fg, &env_fx, *fun_body, Box::new(move |fg, r| {
                        let nc = fg.fresh("nc");
                        cps::Expr::Let(nc.clone(), cps::Val::NullCont,
                            Box::new(cps::Expr::Encore(kv2, vec![r], nc)))
                    }))),
                },
                Box::new(transform(fg, &env_f, *rest, k)),
            )
        }

        dsi::Expr::Ctor(tag, fields) => {
            transform_fields(fg, env, fields, vec![], tag, k)
        }

        dsi::Expr::Field(e, idx) => {
            transform(fg, env, *e, Box::new(move |fg, v| {
                let tmp = fg.fresh("fld");
                cps::Expr::Let(
                    tmp.clone(),
                    cps::Val::Field(v, idx),
                    Box::new(k(fg, tmp)),
                )
            }))
        }

        dsi::Expr::Int(n) => {
            let tmp = fg.fresh("i");
            cps::Expr::Let(
                tmp.clone(),
                cps::Val::Int(n),
                Box::new(k(fg, tmp)),
            )
        }

        dsi::Expr::Prim(op, fields) => {
            transform_prim_fields(fg, env, fields, vec![], op, k)
        }

        dsi::Expr::Extern(slot) => {
            let tmp = fg.fresh("ext");
            cps::Expr::Let(tmp.clone(), cps::Val::Extern(slot), Box::new(k(fg, tmp)))
        }

        dsi::Expr::Match(e, base, cases) => {
            let kn = fg.fresh("k");
            let r = fg.fresh("r");
            let r2 = r.clone();
            let kn2 = kn.clone();
            let env_owned = env.to_vec();
            cps::Expr::Let(
                kn,
                cps::Val::Cont(cps::Cont {
                    param: r,
                    body: Box::new(k(fg, r2)),
                }),
                Box::new(transform(fg, env, *e, Box::new(move |fg, v| {
                    let cps_cases = cases
                        .into_iter()
                        .map(|c| {
                            let kn_ref = kn2.clone();
                            let mut env_case = env_owned.clone();
                            let mut binds = Vec::new();
                            for _ in 0..c.arity {
                                let b = fg.fresh("b");
                                env_case.push(b.clone());
                                binds.push(b);
                            }
                            cps::Case {
                                binds,
                                body: transform(fg, &env_case, c.body, Box::new(move |fg, r| {
                                    let nc = fg.fresh("nc");
                                    cps::Expr::Let(nc.clone(), cps::Val::NullCont,
                                        Box::new(cps::Expr::Encore(kn_ref, vec![r], nc)))
                                })),
                            }
                        })
                        .collect();
                    cps::Expr::Match(v, base, cps_cases)
                }))),
            )
        }
    }
}

fn transform_prim_fields(
    fg: &mut FreshGen,
    env: &[String],
    mut fields: Vec<dsi::Expr>,
    acc: Vec<String>,
    op: PrimOp,
    k: Cont,
) -> cps::Expr {
    if fields.is_empty() {
        let tmp = fg.fresh("p");
        return cps::Expr::Let(
            tmp.clone(),
            cps::Val::Prim(op, acc),
            Box::new(k(fg, tmp)),
        );
    }
    let head = fields.remove(0);
    let env_owned = env.to_vec();
    transform(fg, env, head, Box::new(move |fg, v| {
        let mut acc = acc;
        acc.push(v);
        transform_prim_fields(fg, &env_owned, fields, acc, op, k)
    }))
}

fn transform_appn_args(
    fg: &mut FreshGen,
    env: &[String],
    mut args: Vec<dsi::Expr>,
    acc: Vec<String>,
    f: String,
    k: Cont,
) -> cps::Expr {
    if args.is_empty() {
        let kn = fg.fresh("k");
        let r = fg.fresh("r");
        let r2 = r.clone();
        return cps::Expr::Let(
            kn.clone(),
            cps::Val::Cont(cps::Cont {
                param: r,
                body: Box::new(k(fg, r2)),
            }),
            Box::new(cps::Expr::Encore(f, acc, kn)),
        );
    }
    let head = args.remove(0);
    let env_owned = env.to_vec();
    transform(fg, env, head, Box::new(move |fg, v| {
        let mut acc = acc;
        acc.push(v);
        transform_appn_args(fg, &env_owned, args, acc, f, k)
    }))
}

fn transform_fields(
    fg: &mut FreshGen,
    env: &[String],
    mut fields: Vec<dsi::Expr>,
    acc: Vec<String>,
    tag: u8,
    k: Cont,
) -> cps::Expr {
    if fields.is_empty() {
        let c = fg.fresh("c");
        return cps::Expr::Let(
            c.clone(),
            cps::Val::Ctor(tag, acc),
            Box::new(k(fg, c)),
        );
    }
    let head = fields.remove(0);
    let env_owned = env.to_vec();
    transform(fg, env, head, Box::new(move |fg, v| {
        let mut acc = acc;
        acc.push(v);
        transform_fields(fg, &env_owned, fields, acc, tag, k)
    }))
}
