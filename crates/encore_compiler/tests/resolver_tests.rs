use encore_compiler::ir::{asm, cps};
use encore_compiler::pass::asm_resolve;

use asm::Loc::*;

fn resolve_one(module: &cps::Module) -> asm::Expr {
    let mut ir = asm_resolve::resolve_module(module);
    ir.defines.pop().unwrap().body
}

fn define(name: &str, body: cps::Expr) -> cps::Define {
    cps::Define { name: name.into(), body }
}

fn module(defines: Vec<cps::Define>) -> cps::Module {
    cps::Module { defines }
}

// ASM helpers

fn fin(loc: asm::Loc) -> asm::Expr {
    asm::Expr::Fin(loc)
}

fn let_(val: asm::Val, body: asm::Expr) -> asm::Expr {
    asm::Expr::Let(val, Box::new(body))
}

fn loc(l: asm::Loc) -> asm::Val {
    asm::Val::Loc(l)
}

fn ctor(tag: u8, fields: Vec<asm::Loc>) -> asm::Val {
    asm::Val::Ctor(tag, fields)
}

fn field(l: asm::Loc, idx: u8) -> asm::Val {
    asm::Val::Field(l, idx)
}

fn cont(captures: Vec<asm::Loc>, body: asm::Expr) -> asm::Val {
    asm::Val::ContLam(asm::ContLam { captures, body: Box::new(body) })
}

fn ret(cont: asm::Loc, val: asm::Loc) -> asm::Expr {
    asm::Expr::Return(cont, val)
}

fn encore(f: asm::Loc, arg: asm::Loc, k: asm::Loc) -> asm::Expr {
    asm::Expr::Encore(f, arg, k)
}

fn letrec(captures: Vec<asm::Loc>, body: asm::Expr, rest: asm::Expr) -> asm::Expr {
    asm::Expr::Letrec(
        asm::Fun { captures, body: Box::new(body) },
        Box::new(rest),
    )
}

fn match_(scrutinee: asm::Loc, base: u8, cases: Vec<(u8, asm::Expr)>) -> asm::Expr {
    asm::Expr::Match(
        scrutinee, base,
        cases.into_iter().map(|(arity, body)| asm::Case { arity, body }).collect(),
    )
}

// -- Fin / Global --

#[test]
fn test_halt_global() {
    let m = module(vec![
        define("main", cps::Expr::Fin("main".into())),
    ]);
    assert_eq!(resolve_one(&m), fin(Global(0)));
}

// -- Let with Var --

#[test]
fn test_let_var() {
    let m = module(vec![
        define("g", cps::Expr::Fin("g".into())),
        define("main", cps::Expr::Let(
            "x".into(),
            cps::Val::Var("g".into()),
            Box::new(cps::Expr::Fin("x".into())),
        )),
    ]);
    assert_eq!(resolve_one(&m), let_(loc(Global(0)), fin(Local(0))));
}

// -- Let with Ctor --

#[test]
fn test_let_ctor_nullary() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "c".into(),
            cps::Val::Ctor(5, vec![]),
            Box::new(cps::Expr::Fin("c".into())),
        )),
    ]);
    assert_eq!(resolve_one(&m), let_(ctor(5, vec![]), fin(Local(0))));
}

#[test]
fn test_let_ctor_with_fields() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "a".into(), cps::Val::Ctor(1, vec![]),
            Box::new(cps::Expr::Let(
                "b".into(), cps::Val::Ctor(2, vec![]),
                Box::new(cps::Expr::Let(
                    "pair".into(), cps::Val::Ctor(0, vec!["a".into(), "b".into()]),
                    Box::new(cps::Expr::Fin("pair".into())),
                )),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(ctor(1, vec![]),
            let_(ctor(2, vec![]),
                let_(ctor(0, vec![Local(0), Local(1)]),
                    fin(Local(2)))))
    );
}

// -- Multiple lets, reference order --

#[test]
fn test_multiple_lets_ref_first() {
    let m = module(vec![
        define("g0", cps::Expr::Fin("g0".into())),
        define("g1", cps::Expr::Fin("g1".into())),
        define("main", cps::Expr::Let(
            "x".into(), cps::Val::Var("g0".into()),
            Box::new(cps::Expr::Let(
                "y".into(), cps::Val::Var("g1".into()),
                Box::new(cps::Expr::Fin("x".into())),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(loc(Global(0)), let_(loc(Global(1)), fin(Local(0))))
    );
}

#[test]
fn test_multiple_lets_ref_second() {
    let m = module(vec![
        define("g0", cps::Expr::Fin("g0".into())),
        define("g1", cps::Expr::Fin("g1".into())),
        define("main", cps::Expr::Let(
            "x".into(), cps::Val::Var("g0".into()),
            Box::new(cps::Expr::Let(
                "y".into(), cps::Val::Var("g1".into()),
                Box::new(cps::Expr::Fin("y".into())),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(loc(Global(0)), let_(loc(Global(1)), fin(Local(1))))
    );
}

// -- Let with Field --

#[test]
fn test_let_field() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "a".into(), cps::Val::Ctor(1, vec![]),
            Box::new(cps::Expr::Let(
                "b".into(), cps::Val::Ctor(2, vec![]),
                Box::new(cps::Expr::Let(
                    "pair".into(), cps::Val::Ctor(0, vec!["a".into(), "b".into()]),
                    Box::new(cps::Expr::Let(
                        "snd".into(), cps::Val::Field("pair".into(), 1),
                        Box::new(cps::Expr::Fin("snd".into())),
                    )),
                )),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(ctor(1, vec![]),
            let_(ctor(2, vec![]),
                let_(ctor(0, vec![Local(0), Local(1)]),
                    let_(field(Local(2), 1),
                        fin(Local(3))))))
    );
}

// -- Cont: identity continuation --

#[test]
fn test_cont_identity() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "f".into(),
            cps::Val::Cont(cps::Cont {
                param: "x".into(),
                body: Box::new(cps::Expr::Fin("x".into())),
            }),
            Box::new(cps::Expr::Return("f".into(), "main".into())),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(cont(vec![], fin(Arg)), ret(Local(0), Global(0)))
    );
}

// -- Cont: global accessed directly (not captured) --

#[test]
fn test_cont_global_not_captured() {
    let m = module(vec![
        define("g", cps::Expr::Fin("g".into())),
        define("main", cps::Expr::Let(
            "f".into(),
            cps::Val::Cont(cps::Cont {
                param: "x".into(),
                body: Box::new(cps::Expr::Fin("g".into())),
            }),
            Box::new(cps::Expr::Return("f".into(), "main".into())),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(cont(vec![], fin(Global(0))), ret(Local(0), Global(1)))
    );
}

// -- Cont: captures a local --

#[test]
fn test_cont_captures_local() {
    let m = module(vec![
        define("g0", cps::Expr::Fin("g0".into())),
        define("g1", cps::Expr::Fin("g1".into())),
        define("main", cps::Expr::Let(
            "v".into(), cps::Val::Var("g0".into()),
            Box::new(cps::Expr::Let(
                "f".into(),
                cps::Val::Cont(cps::Cont {
                    param: "x".into(),
                    body: Box::new(cps::Expr::Fin("v".into())),
                }),
                Box::new(cps::Expr::Return("f".into(), "g1".into())),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(loc(Global(0)),
            let_(cont(vec![Local(0)], fin(Capture(0))),
                ret(Local(1), Global(1))))
    );
}

// -- Cont: captures multiple locals --

#[test]
fn test_cont_captures_two_locals() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "a".into(), cps::Val::Ctor(1, vec![]),
            Box::new(cps::Expr::Let(
                "b".into(), cps::Val::Ctor(2, vec![]),
                Box::new(cps::Expr::Let(
                    "f".into(),
                    cps::Val::Cont(cps::Cont {
                        param: "x".into(),
                        body: Box::new(cps::Expr::Let(
                            "pair".into(),
                            cps::Val::Ctor(0, vec!["a".into(), "b".into()]),
                            Box::new(cps::Expr::Fin("pair".into())),
                        )),
                    }),
                    Box::new(cps::Expr::Return("f".into(), "main".into())),
                )),
            )),
        )),
    ]);
    let asm = resolve_one(&m);
    assert_eq!(
        asm,
        let_(ctor(1, vec![]),
            let_(ctor(2, vec![]),
                let_(cont(vec![Local(0), Local(1)],
                        let_(ctor(0, vec![Capture(0), Capture(1)]),
                            fin(Local(0)))),
                    ret(Local(2), Global(0)))))
    );
}

// -- Match: nullary branches --

#[test]
fn test_match_branch0() {
    let m = module(vec![
        define("g0", cps::Expr::Fin("g0".into())),
        define("g1", cps::Expr::Fin("g1".into())),
        define("g2", cps::Expr::Fin("g2".into())),
        define("main", cps::Expr::Let(
            "c".into(), cps::Val::Var("g0".into()),
            Box::new(cps::Expr::Match("c".into(), 0, vec![
                cps::Case { binds: vec![], body: cps::Expr::Fin("g1".into()) },
                cps::Case { binds: vec![], body: cps::Expr::Fin("g2".into()) },
            ])),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(loc(Global(0)),
            match_(Local(0), 0, vec![
                (0, fin(Global(1))),
                (0, fin(Global(2))),
            ]))
    );
}

#[test]
fn test_match_branch1() {
    let m = module(vec![
        define("g0", cps::Expr::Fin("g0".into())),
        define("g1", cps::Expr::Fin("g1".into())),
        define("g2", cps::Expr::Fin("g2".into())),
        define("main", cps::Expr::Let(
            "c".into(), cps::Val::Var("g0".into()),
            Box::new(cps::Expr::Match("c".into(), 0, vec![
                cps::Case { binds: vec![], body: cps::Expr::Fin("g1".into()) },
                cps::Case { binds: vec![], body: cps::Expr::Fin("g2".into()) },
            ])),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(loc(Global(0)),
            match_(Local(0), 0, vec![
                (0, fin(Global(1))),
                (0, fin(Global(2))),
            ]))
    );
}

// -- Match: field extraction via binds --

#[test]
fn test_match_with_binds() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "a".into(), cps::Val::Ctor(1, vec![]),
            Box::new(cps::Expr::Let(
                "b".into(), cps::Val::Ctor(2, vec![]),
                Box::new(cps::Expr::Let(
                    "pair".into(), cps::Val::Ctor(0, vec!["a".into(), "b".into()]),
                    Box::new(cps::Expr::Match("pair".into(), 0, vec![
                        cps::Case {
                            binds: vec!["fst".into(), "snd".into()],
                            body: cps::Expr::Fin("snd".into()),
                        },
                    ])),
                )),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(ctor(1, vec![]),
            let_(ctor(2, vec![]),
                let_(ctor(0, vec![Local(0), Local(1)]),
                    match_(Local(2), 0, vec![
                        (2, fin(Local(4))),
                    ]))))
    );
}

#[test]
fn test_match_binds_first_field() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "a".into(), cps::Val::Ctor(1, vec![]),
            Box::new(cps::Expr::Let(
                "b".into(), cps::Val::Ctor(2, vec![]),
                Box::new(cps::Expr::Let(
                    "pair".into(), cps::Val::Ctor(0, vec!["a".into(), "b".into()]),
                    Box::new(cps::Expr::Match("pair".into(), 0, vec![
                        cps::Case {
                            binds: vec!["fst".into(), "snd".into()],
                            body: cps::Expr::Fin("fst".into()),
                        },
                    ])),
                )),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(ctor(1, vec![]),
            let_(ctor(2, vec![]),
                let_(ctor(0, vec![Local(0), Local(1)]),
                    match_(Local(2), 0, vec![
                        (2, fin(Local(3))),
                    ]))))
    );
}

// -- Letrec: simple function --

#[test]
fn test_letrec_simple() {
    let m = module(vec![
        define("main", cps::Expr::Letrec(
            "f".into(),
            cps::Fun {
                arg: "x".into(),
                cont: "k".into(),
                body: Box::new(cps::Expr::Return("k".into(), "x".into())),
            },
            Box::new(cps::Expr::Let(
                "k0".into(),
                cps::Val::Cont(cps::Cont {
                    param: "r".into(),
                    body: Box::new(cps::Expr::Fin("r".into())),
                }),
                Box::new(cps::Expr::Encore("f".into(), "main".into(), "k0".into())),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        letrec(vec![], ret(Cont, Arg),
            let_(cont(vec![], fin(Arg)),
                encore(Local(0), Global(0), Local(1))))
    );
}

// -- Peano countdown --

#[test]
fn test_peano_countdown() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "zero".into(), cps::Val::Ctor(0, vec![]),
            Box::new(cps::Expr::Let(
                "s1".into(), cps::Val::Ctor(1, vec!["zero".into()]),
                Box::new(cps::Expr::Let(
                    "s2".into(), cps::Val::Ctor(1, vec!["s1".into()]),
                    Box::new(cps::Expr::Letrec(
                        "f".into(),
                        cps::Fun {
                            arg: "n".into(),
                            cont: "k".into(),
                            body: Box::new(cps::Expr::Match("n".into(), 0, vec![
                                cps::Case {
                                    binds: vec![],
                                    body: cps::Expr::Return("k".into(), "n".into()),
                                },
                                cps::Case {
                                    binds: vec!["pred".into()],
                                    body: cps::Expr::Encore("f".into(), "pred".into(), "k".into()),
                                },
                            ])),
                        },
                        Box::new(cps::Expr::Let(
                            "k0".into(),
                            cps::Val::Cont(cps::Cont {
                                param: "r".into(),
                                body: Box::new(cps::Expr::Fin("r".into())),
                            }),
                            Box::new(cps::Expr::Encore("f".into(), "s2".into(), "k0".into())),
                        )),
                    )),
                )),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(ctor(0, vec![]),
            let_(ctor(1, vec![Local(0)]),
                let_(ctor(1, vec![Local(1)]),
                    letrec(vec![],
                        match_(Arg, 0, vec![
                            (0, ret(Cont, Arg)),
                            (1, encore(SelfRef, Local(0), Cont)),
                        ]),
                        let_(cont(vec![], fin(Arg)),
                            encore(Local(3), Local(2), Local(4)))))))
    );
}

// -- Peano countdown from 5 --

#[test]
fn test_peano_countdown_5() {
    let m = module(vec![
        define("main", cps::Expr::Let(
            "z".into(), cps::Val::Ctor(0, vec![]),
            Box::new(cps::Expr::Let(
                "s1".into(), cps::Val::Ctor(1, vec!["z".into()]),
                Box::new(cps::Expr::Let(
                    "s2".into(), cps::Val::Ctor(1, vec!["s1".into()]),
                    Box::new(cps::Expr::Let(
                        "s3".into(), cps::Val::Ctor(1, vec!["s2".into()]),
                        Box::new(cps::Expr::Let(
                            "s4".into(), cps::Val::Ctor(1, vec!["s3".into()]),
                            Box::new(cps::Expr::Let(
                                "s5".into(), cps::Val::Ctor(1, vec!["s4".into()]),
                                Box::new(cps::Expr::Letrec(
                                    "f".into(),
                                    cps::Fun {
                                        arg: "n".into(),
                                        cont: "k".into(),
                                        body: Box::new(cps::Expr::Match("n".into(), 0, vec![
                                            cps::Case {
                                                binds: vec![],
                                                body: cps::Expr::Return("k".into(), "n".into()),
                                            },
                                            cps::Case {
                                                binds: vec!["pred".into()],
                                                body: cps::Expr::Encore("f".into(), "pred".into(), "k".into()),
                                            },
                                        ])),
                                    },
                                    Box::new(cps::Expr::Let(
                                        "k0".into(),
                                        cps::Val::Cont(cps::Cont {
                                            param: "r".into(),
                                            body: Box::new(cps::Expr::Fin("r".into())),
                                        }),
                                        Box::new(cps::Expr::Encore("f".into(), "s5".into(), "k0".into())),
                                    )),
                                )),
                            )),
                        )),
                    )),
                )),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(ctor(0, vec![]),
            let_(ctor(1, vec![Local(0)]),
                let_(ctor(1, vec![Local(1)]),
                    let_(ctor(1, vec![Local(2)]),
                        let_(ctor(1, vec![Local(3)]),
                            let_(ctor(1, vec![Local(4)]),
                                letrec(vec![],
                                    match_(Arg, 0, vec![
                                        (0, ret(Cont, Arg)),
                                        (1, encore(SelfRef, Local(0), Cont)),
                                    ]),
                                    let_(cont(vec![], fin(Arg)),
                                        encore(Local(6), Local(5), Local(7))))))))))
    );
}

// -- Cont called with different arg than capture --

#[test]
fn test_cont_capture_vs_arg() {
    let m = module(vec![
        define("g0", cps::Expr::Fin("g0".into())),
        define("g1", cps::Expr::Fin("g1".into())),
        define("main", cps::Expr::Let(
            "v".into(), cps::Val::Var("g0".into()),
            Box::new(cps::Expr::Let(
                "f".into(),
                cps::Val::Cont(cps::Cont {
                    param: "x".into(),
                    body: Box::new(cps::Expr::Let(
                        "pair".into(), cps::Val::Ctor(0, vec!["v".into(), "x".into()]),
                        Box::new(cps::Expr::Let(
                            "result".into(), cps::Val::Field("pair".into(), 0),
                            Box::new(cps::Expr::Fin("result".into())),
                        )),
                    )),
                }),
                Box::new(cps::Expr::Return("f".into(), "g1".into())),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(loc(Global(0)),
            let_(cont(vec![Local(0)],
                    let_(ctor(0, vec![Capture(0), Arg]),
                        let_(field(Local(0), 0),
                            fin(Local(1))))),
                ret(Local(1), Global(1))))
    );
}

// -- Nested cont: inner captures from outer --

#[test]
fn test_nested_cont() {
    let m = module(vec![
        define("g", cps::Expr::Fin("g".into())),
        define("main", cps::Expr::Let(
            "outer".into(),
            cps::Val::Cont(cps::Cont {
                param: "x".into(),
                body: Box::new(cps::Expr::Let(
                    "inner".into(),
                    cps::Val::Cont(cps::Cont {
                        param: "y".into(),
                        body: Box::new(cps::Expr::Fin("x".into())),
                    }),
                    Box::new(cps::Expr::Return("inner".into(), "g".into())),
                )),
            }),
            Box::new(cps::Expr::Return("outer".into(), "g".into())),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(cont(vec![],
                let_(cont(vec![Arg], fin(Capture(0))),
                    ret(Local(0), Global(0)))),
            ret(Local(0), Global(0)))
    );
}
