use crate::ir::{cps, ds, prim::PrimOp};

pub const CONT_TAG: u8 = 255;

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

pub fn transform_module(module: ds::Module) -> cps::Module {
    cps::Module {
        defines: module
            .defines
            .into_iter()
            .map(|d| {
                let mut fg = FreshGen::new();
                cps::Define {
                    name: d.name,
                    body: transform(&mut fg, d.body, Box::new(halt)),
                }
            })
            .collect(),
    }
}

fn transform(fg: &mut FreshGen, expr: ds::Expr, k: Cont) -> cps::Expr {
    match expr {
        ds::Expr::Var(x) => k(fg, x),

        ds::Expr::Lam(x, body) => {
            let f = fg.fresh("f");
            let p = fg.fresh("p");
            let kv = fg.fresh("k");
            let kv2 = kv.clone();
            cps::Expr::Let(
                f.clone(),
                cps::Val::Lambda(cps::Lambda {
                    param: p.clone(),
                    body: Box::new(cps::Expr::Let(
                        x,
                        cps::Val::Field(p.clone(), 0),
                        Box::new(cps::Expr::Let(
                            kv,
                            cps::Val::Field(p, 1),
                            Box::new(transform(fg, *body, Box::new(move |_fg, r| {
                                cps::Expr::App(kv2, r)
                            }))),
                        )),
                    )),
                }),
                Box::new(k(fg, f)),
            )
        }

        ds::Expr::App(e1, e2) => {
            transform(fg, *e1, Box::new(move |fg, f| {
                transform(fg, *e2, Box::new(move |fg, x| {
                    let kn = fg.fresh("k");
                    let r = fg.fresh("r");
                    let pair = fg.fresh("pair");
                    let r2 = r.clone();
                    cps::Expr::Let(
                        kn.clone(),
                        cps::Val::Lambda(cps::Lambda {
                            param: r,
                            body: Box::new(k(fg, r2)),
                        }),
                        Box::new(cps::Expr::Let(
                            pair.clone(),
                            cps::Val::Ctor(CONT_TAG, vec![x, kn]),
                            Box::new(cps::Expr::App(f, pair)),
                        )),
                    )
                }))
            }))
        }

        ds::Expr::Let(x, e1, e2) => {
            transform(fg, *e1, Box::new(move |fg, v| {
                cps::Expr::Let(
                    x,
                    cps::Val::Var(v),
                    Box::new(transform(fg, *e2, k)),
                )
            }))
        }

        ds::Expr::Letrec(f, x, body, rest) => {
            let p = fg.fresh("p");
            let kv = fg.fresh("k");
            let kv2 = kv.clone();
            cps::Expr::Letrec(
                f,
                cps::Lambda {
                    param: p.clone(),
                    body: Box::new(cps::Expr::Let(
                        x,
                        cps::Val::Field(p.clone(), 0),
                        Box::new(cps::Expr::Let(
                            kv,
                            cps::Val::Field(p, 1),
                            Box::new(transform(fg, *body, Box::new(move |_fg, r| {
                                cps::Expr::App(kv2, r)
                            }))),
                        )),
                    )),
                },
                Box::new(transform(fg, *rest, k)),
            )
        }

        ds::Expr::Ctor(tag, fields) => {
            transform_fields(fg, fields, vec![], tag, k)
        }

        ds::Expr::Field(e, idx) => {
            transform(fg, *e, Box::new(move |fg, v| {
                let tmp = fg.fresh("fld");
                cps::Expr::Let(
                    tmp.clone(),
                    cps::Val::Field(v, idx),
                    Box::new(k(fg, tmp)),
                )
            }))
        }

        ds::Expr::Int(n) => {
            let tmp = fg.fresh("i");
            cps::Expr::Let(
                tmp.clone(),
                cps::Val::Int(n),
                Box::new(k(fg, tmp)),
            )
        }

        ds::Expr::Prim(op, fields) => {
            transform_prim_fields(fg, fields, vec![], op, k)
        }

        ds::Expr::Match(e, base, cases) => {
            let kn = fg.fresh("k");
            let r = fg.fresh("r");
            let r2 = r.clone();
            let kn2 = kn.clone();
            cps::Expr::Let(
                kn,
                cps::Val::Lambda(cps::Lambda {
                    param: r,
                    body: Box::new(k(fg, r2)),
                }),
                Box::new(transform(fg, *e, Box::new(move |fg, v| {
                    let cps_cases = cases
                        .into_iter()
                        .map(|c| {
                            let kn_ref = kn2.clone();
                            cps::Case {
                                binds: c.binds,
                                body: transform(fg, c.body, Box::new(move |_fg, r| {
                                    cps::Expr::App(kn_ref, r)
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
    mut fields: Vec<ds::Expr>,
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
    transform(fg, head, Box::new(move |fg, v| {
        let mut acc = acc;
        acc.push(v);
        transform_prim_fields(fg, fields, acc, op, k)
    }))
}

fn transform_fields(
    fg: &mut FreshGen,
    mut fields: Vec<ds::Expr>,
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
    transform(fg, head, Box::new(move |fg, v| {
        let mut acc = acc;
        acc.push(v);
        transform_fields(fg, fields, acc, tag, k)
    }))
}
