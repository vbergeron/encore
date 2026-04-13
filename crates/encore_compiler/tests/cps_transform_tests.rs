use encore_compiler::ir::{cps, ds};
use encore_compiler::pass::{cps_transform, dsi_resolve};

fn transform(module: ds::Module) -> cps::Module {
    let dsi_module = dsi_resolve::resolve_module(module);
    cps_transform::transform_module(dsi_module)
}

fn transform_one(module: ds::Module) -> cps::Expr {
    let mut cps_module = transform(module);
    assert_eq!(cps_module.defines.len(), 1);
    cps_module.defines.remove(0).body
}

// DS helpers

fn define(name: &str, body: ds::Expr) -> ds::Define {
    ds::Define { name: name.into(), body }
}

fn module(defines: Vec<ds::Define>) -> ds::Module {
    ds::Module { defines }
}

fn var(x: &str) -> ds::Expr {
    ds::Expr::Var(x.into())
}

fn lam(x: &str, body: ds::Expr) -> ds::Expr {
    ds::Expr::Lam(x.into(), Box::new(body))
}

fn app(f: ds::Expr, x: ds::Expr) -> ds::Expr {
    ds::Expr::App(Box::new(f), Box::new(x))
}

fn ds_let(x: &str, e1: ds::Expr, e2: ds::Expr) -> ds::Expr {
    ds::Expr::Let(x.into(), Box::new(e1), Box::new(e2))
}

fn letrec(f: &str, x: &str, body: ds::Expr, rest: ds::Expr) -> ds::Expr {
    ds::Expr::Letrec(f.into(), x.into(), Box::new(body), Box::new(rest))
}

fn ds_ctor(tag: u8, fields: Vec<ds::Expr>) -> ds::Expr {
    ds::Expr::Ctor(tag, fields)
}

fn field(e: ds::Expr, idx: u8) -> ds::Expr {
    ds::Expr::Field(Box::new(e), idx)
}

fn ds_match(e: ds::Expr, base: u8, cases: Vec<ds::Case>) -> ds::Expr {
    ds::Expr::Match(Box::new(e), base, cases)
}

fn case(binds: Vec<&str>, body: ds::Expr) -> ds::Case {
    ds::Case {
        binds: binds.into_iter().map(String::from).collect(),
        body,
    }
}

fn count_ctor(expr: &cps::Expr, tag: u8) -> usize {
    match expr {
        cps::Expr::Fin(_) => 0,
        cps::Expr::Let(_, val, body) => {
            let here = match val {
                cps::Val::Ctor(t, _) if *t == tag => 1,
                cps::Val::Cont(cont) => count_ctor(&cont.body, tag),
                _ => 0,
            };
            here + count_ctor(body, tag)
        }
        cps::Expr::Letrec(_, fun, body) => {
            count_ctor(&fun.body, tag) + count_ctor(body, tag)
        }
        cps::Expr::Encore(_, _, _) => 0,
        cps::Expr::Return(_, _) => 0,
        cps::Expr::Match(_, _, cases) => {
            cases.iter().map(|c| count_ctor(&c.body, tag)).sum()
        }
    }
}

fn has_letrec(expr: &cps::Expr) -> bool {
    match expr {
        cps::Expr::Letrec(_, _, _) => true,
        cps::Expr::Let(_, val, body) => {
            let in_val = match val {
                cps::Val::Cont(cont) => has_letrec(&cont.body),
                _ => false,
            };
            in_val || has_letrec(body)
        }
        cps::Expr::Match(_, _, cases) => cases.iter().any(|c| has_letrec(&c.body)),
        _ => false,
    }
}

fn has_encore(expr: &cps::Expr) -> bool {
    match expr {
        cps::Expr::Encore(_, _, _) => true,
        cps::Expr::Let(_, val, body) => {
            let in_val = match val {
                cps::Val::Cont(cont) => has_encore(&cont.body),
                _ => false,
            };
            in_val || has_encore(body)
        }
        cps::Expr::Letrec(_, fun, body) => has_encore(&fun.body) || has_encore(body),
        cps::Expr::Match(_, _, cases) => cases.iter().any(|c| has_encore(&c.body)),
        _ => false,
    }
}

fn has_match(expr: &cps::Expr, base: u8, n_cases: usize) -> bool {
    match expr {
        cps::Expr::Match(_, b, cases) => *b == base && cases.len() == n_cases,
        cps::Expr::Let(_, val, body) => {
            let in_val = match val {
                cps::Val::Cont(cont) => has_match(&cont.body, base, n_cases),
                _ => false,
            };
            in_val || has_match(body, base, n_cases)
        }
        cps::Expr::Letrec(_, fun, body) => {
            has_match(&fun.body, base, n_cases) || has_match(body, base, n_cases)
        }
        _ => false,
    }
}

// -- Trivial: just return a global --

#[test]
fn test_var() {
    let m = module(vec![define("main", var("main"))]);
    let cps = transform_one(m);
    assert_eq!(cps, cps::Expr::Fin("main".into()));
}

// -- Let + Var: optimizer may inline trivial let --

#[test]
fn test_let_var() {
    let m = module(vec![define("main", ds_let("x", var("main"), var("x")))]);
    let cps = transform_one(m);

    // Either inlined to Fin("main") or Let(_x, Var("main"), Fin(_x))
    match &cps {
        cps::Expr::Fin(name) => assert_eq!(name, "main"),
        cps::Expr::Let(_, cps::Val::Var(src), body) => {
            assert_eq!(src, "main");
            assert!(matches!(body.as_ref(), cps::Expr::Fin(_)));
        }
        other => panic!("unexpected: {other:?}"),
    }
}

// -- Ctor --

#[test]
fn test_ctor_nullary() {
    let m = module(vec![define("main", ds_let("c", ds_ctor(5, vec![]), var("c")))]);
    let cps = transform_one(m);

    assert_eq!(count_ctor(&cps, 5), 1);
    // Should end with Fin
    match &cps {
        cps::Expr::Let(name, cps::Val::Ctor(5, fields), body) => {
            assert!(fields.is_empty());
            assert_eq!(body.as_ref(), &cps::Expr::Fin(name.clone()));
        }
        other => panic!("expected Let(Ctor(5,..)), got {other:?}"),
    }
}

#[test]
fn test_ctor_nested() {
    // Ctor(0, [Ctor(1, []), Ctor(2, [])])
    let m = module(vec![
        define("main", ds_ctor(0, vec![ds_ctor(1, vec![]), ds_ctor(2, vec![])])),
    ]);
    let cps = transform_one(m);

    assert_eq!(count_ctor(&cps, 0), 1);
    assert_eq!(count_ctor(&cps, 1), 1);
    assert_eq!(count_ctor(&cps, 2), 1);
}

// -- Field --

#[test]
fn test_field_of_ctor() {
    // field(Ctor(0, [Ctor(7, [])]), 0)
    let m = module(vec![
        define("main", field(ds_ctor(0, vec![ds_ctor(7, vec![])]), 0)),
    ]);
    let cps = transform_one(m);

    assert_eq!(count_ctor(&cps, 7), 1);
    assert_eq!(count_ctor(&cps, 0), 1);
}

#[test]
fn test_field_second() {
    // field(Ctor(0, [Ctor(1, []), Ctor(2, [])]), 1)
    let m = module(vec![
        define("main", field(ds_ctor(0, vec![ds_ctor(1, vec![]), ds_ctor(2, vec![])]), 1)),
    ]);
    let cps = transform_one(m);

    assert_eq!(count_ctor(&cps, 0), 1);
    assert_eq!(count_ctor(&cps, 1), 1);
    assert_eq!(count_ctor(&cps, 2), 1);
}

// -- Identity function --

#[test]
fn test_identity() {
    // let id = \x -> x in id(Ctor(42, []))
    let m = module(vec![
        define("main", ds_let(
            "id", lam("x", var("x")),
            app(var("id"), ds_ctor(42, vec![])),
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_letrec(&cps));
    assert!(has_encore(&cps));
    assert_eq!(count_ctor(&cps, 42), 1);
}

// -- Constant function --

#[test]
fn test_constant_fn() {
    // let k = \x -> Ctor(10, []) in k(Ctor(99, []))
    let m = module(vec![
        define("main", ds_let(
            "k", lam("x", ds_ctor(10, vec![])),
            app(var("k"), ds_ctor(99, vec![])),
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_letrec(&cps));
    assert_eq!(count_ctor(&cps, 10), 1);
    assert_eq!(count_ctor(&cps, 99), 1);
}

// -- Nested application: f(g(x)) --

#[test]
fn test_nested_app() {
    // let id = \x -> x in id(id(Ctor(33, [])))
    let m = module(vec![
        define("main", ds_let(
            "id", lam("x", var("x")),
            app(var("id"), app(var("id"), ds_ctor(33, vec![]))),
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_letrec(&cps));
    assert_eq!(count_ctor(&cps, 33), 1);
}

// -- Lambda capturing a local --

#[test]
fn test_lambda_capture() {
    // let v = Ctor(15, []) in let f = \x -> v in f(Ctor(99, []))
    let m = module(vec![
        define("main", ds_let(
            "v", ds_ctor(15, vec![]),
            ds_let(
                "f", lam("x", var("v")),
                app(var("f"), ds_ctor(99, vec![])),
            ),
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_letrec(&cps));
    assert_eq!(count_ctor(&cps, 15), 1);
}

// -- Match --

#[test]
fn test_match_branch0() {
    // match Ctor(0, []) base=0 { [] -> Ctor(10, []), [] -> Ctor(20, []) }
    let m = module(vec![
        define("main", ds_match(
            ds_ctor(0, vec![]),
            0,
            vec![
                case(vec![], ds_ctor(10, vec![])),
                case(vec![], ds_ctor(20, vec![])),
            ],
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_match(&cps, 0, 2));
    assert_eq!(count_ctor(&cps, 0), 1);
    assert_eq!(count_ctor(&cps, 10), 1);
    assert_eq!(count_ctor(&cps, 20), 1);
}

#[test]
fn test_match_branch1() {
    // match Ctor(1, []) base=0 { [] -> Ctor(10, []), [] -> Ctor(20, []) }
    let m = module(vec![
        define("main", ds_match(
            ds_ctor(1, vec![]),
            0,
            vec![
                case(vec![], ds_ctor(10, vec![])),
                case(vec![], ds_ctor(20, vec![])),
            ],
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_match(&cps, 0, 2));
    assert_eq!(count_ctor(&cps, 1), 1);
}

#[test]
fn test_match_with_binds() {
    // let pair = Ctor(0, [Ctor(1, []), Ctor(2, [])]) in
    // match pair base=0 { [fst, snd] -> snd }
    let m = module(vec![
        define("main", ds_let(
            "pair", ds_ctor(0, vec![ds_ctor(1, vec![]), ds_ctor(2, vec![])]),
            ds_match(
                var("pair"),
                0,
                vec![case(vec!["fst", "snd"], var("snd"))],
            ),
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_match(&cps, 0, 1));
    assert_eq!(count_ctor(&cps, 0), 1);
    assert_eq!(count_ctor(&cps, 1), 1);
    assert_eq!(count_ctor(&cps, 2), 1);
}

// -- Letrec: Peano countdown --

#[test]
fn test_peano_countdown() {
    let m = module(vec![
        define("main", letrec(
            "countdown", "n",
            ds_match(
                var("n"), 0,
                vec![
                    case(vec![], var("n")),
                    case(vec!["pred"], app(var("countdown"), var("pred"))),
                ],
            ),
            app(
                var("countdown"),
                ds_ctor(1, vec![ds_ctor(1, vec![ds_ctor(0, vec![])])]),
            ),
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_letrec(&cps));
    assert!(has_match(&cps, 0, 2));
    assert!(has_encore(&cps));
}

// -- App with ctor arg --

#[test]
fn test_app_ctor_arg() {
    let m = module(vec![
        define("main", ds_let(
            "fst", lam("pair", field(var("pair"), 0)),
            app(var("fst"), ds_ctor(0, vec![ds_ctor(5, vec![]), ds_ctor(9, vec![])])),
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_letrec(&cps));
    assert!(has_encore(&cps));
    assert_eq!(count_ctor(&cps, 5), 1);
    assert_eq!(count_ctor(&cps, 9), 1);
}

// -- Triple nested app: f(f(f(x))) --

#[test]
fn test_triple_nested_app() {
    let m = module(vec![
        define("main", ds_let(
            "id", lam("x", var("x")),
            app(var("id"), app(var("id"), app(var("id"), ds_ctor(77, vec![])))),
        )),
    ]);
    let cps = transform_one(m);

    assert!(has_letrec(&cps));
    assert_eq!(count_ctor(&cps, 77), 1);
}
