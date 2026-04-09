use encore_compiler::ir::cps;
use encore_compiler::pass::{emit::Emitter, resolver};
use encore_vm::program::Program;
use encore_vm::value::{HeapAddress, Value};
use encore_vm::vm::Vm;

fn ctor(tag: u8) -> Value {
    Value::ctor(tag, HeapAddress::NULL)
}

fn run_define(module: &cps::Module, define_idx: usize, globals: &[Value]) -> Value {
    let ir_module = resolver::resolve_module(module);
    let define = &ir_module.defines[define_idx];
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&define.body);
    let binary = emitter.serialize(globals.len() as u16);
    let prog = Program::parse(&binary).unwrap();
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(prog.code, prog.arity_table, globals, &mut mem);
    vm.run().unwrap()
}

fn define(name: &str, body: cps::Expr) -> cps::Define {
    cps::Define { name: name.into(), body }
}

fn module(defines: Vec<cps::Define>) -> cps::Module {
    cps::Module { defines }
}

// -- Fin / Global --

#[test]
fn test_halt_global() {
    // define main = halt("main")
    let m = module(vec![
        define("main", cps::Expr::Fin("main".into())),
    ]);
    let result = run_define(&m, 0, &[ctor(42)]);
    assert_eq!(result.ctor_tag(), 42);
}

// -- Let with Var --

#[test]
fn test_let_var() {
    // define g = ...; define main = let x = var("g"); halt("x")
    let m = module(vec![
        define("g", cps::Expr::Fin("g".into())),
        define("main", cps::Expr::Let(
            "x".into(),
            cps::Val::Var("g".into()),
            Box::new(cps::Expr::Fin("x".into())),
        )),
    ]);
    let result = run_define(&m, 1, &[ctor(10), ctor(0)]);
    assert_eq!(result.ctor_tag(), 10);
}

// -- Let with Ctor --

#[test]
fn test_let_ctor_nullary() {
    // define main = let c = ctor(5, []); halt("c")
    let m = module(vec![
        define("main", cps::Expr::Let(
            "c".into(),
            cps::Val::Ctor(5, vec![]),
            Box::new(cps::Expr::Fin("c".into())),
        )),
    ]);
    let result = run_define(&m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 5);
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
    let result = run_define(&m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 0);
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
    let result = run_define(&m, 2, &[ctor(10), ctor(20), ctor(0)]);
    assert_eq!(result.ctor_tag(), 10);
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
    let result = run_define(&m, 2, &[ctor(10), ctor(20), ctor(0)]);
    assert_eq!(result.ctor_tag(), 20);
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
    let result = run_define(&m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 2);
}

// -- Cont: identity continuation --

#[test]
fn test_cont_identity() {
    // define main = let f = cont(x). fin x in return f main
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
    let result = run_define(&m, 0, &[ctor(42)]);
    assert_eq!(result.ctor_tag(), 42);
}

// -- Cont: global accessed directly (not captured) --

#[test]
fn test_cont_global_not_captured() {
    // define g = ...; define main = let f = cont(x). fin g in return f main
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
    let result = run_define(&m, 1, &[ctor(10), ctor(99)]);
    assert_eq!(result.ctor_tag(), 10);
}

// -- Cont: captures a local --

#[test]
fn test_cont_captures_local() {
    // define g0 = ...; define g1 = ...
    // define main = let v = var("g0"); let f = cont(x). fin v in return f g1
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
    let result = run_define(&m, 2, &[ctor(10), ctor(20), ctor(0)]);
    assert_eq!(result.ctor_tag(), 10);
}

// -- Cont: captures multiple locals (sorted deterministically) --

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
    let result = run_define(&m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 0);
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
    let result = run_define(&m, 3, &[ctor(0), ctor(10), ctor(20), ctor(0)]);
    assert_eq!(result.ctor_tag(), 10);
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
    let result = run_define(&m, 3, &[ctor(1), ctor(10), ctor(20), ctor(0)]);
    assert_eq!(result.ctor_tag(), 20);
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
    let result = run_define(&m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 2);
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
    let result = run_define(&m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 1);
}

// -- Letrec: simple function that returns its arg via continuation --

#[test]
fn test_letrec_simple() {
    // define main =
    //   letrec f = fun(x, k). return k x
    //   in let k0 = cont(r). fin r
    //   in encore f main k0
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
    let result = run_define(&m, 0, &[ctor(42)]);
    assert_eq!(result.ctor_tag(), 42);
}

// -- Peano countdown: letrec + match + recursion --

#[test]
fn test_peano_countdown() {
    // define main =
    //   let zero = ctor(0, []);
    //   let s1 = ctor(1, ["zero"]);
    //   let s2 = ctor(1, ["s1"]);
    //   letrec f = fun(n, k).
    //     match "n" base=0 [
    //       Case { binds: [], body: return k n },
    //       Case { binds: ["pred"], body: encore f pred k }
    //     ]
    //   in let k0 = cont(r). fin r
    //   in encore f s2 k0
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
    let result = run_define(&m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 0);
}

// -- Peano countdown from 5: stress test --

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
    let result = run_define(&m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 0);
}

// -- Cont called with different arg than capture --

#[test]
fn test_cont_capture_vs_arg() {
    // define g0 = ...; define g1 = ...
    // define main =
    //   let v = var("g0");
    //   let f = cont(x).
    //     let pair = ctor(0, ["v", "x"]);
    //     let result = field("pair", 0);
    //     fin result
    //   in return f g1
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
    let result = run_define(&m, 2, &[ctor(10), ctor(20), ctor(0)]);
    assert_eq!(result.ctor_tag(), 10);
}

// -- Nested cont: inner captures from outer --

#[test]
fn test_nested_cont() {
    // define g = ...
    // define main =
    //   let outer = cont(x).
    //     let inner = cont(y). fin x
    //     in return inner g
    //   in return outer g
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
    let result = run_define(&m, 1, &[ctor(10), ctor(0)]);
    assert_eq!(result.ctor_tag(), 10);
}
