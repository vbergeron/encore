use encore_compiler::ir::{asm, cps};
use encore_compiler::pass::asm_resolve;

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

fn fin(r: asm::Reg) -> asm::Expr {
    asm::Expr::Fin(r)
}

fn let_(dest: asm::Reg, val: asm::Val, body: asm::Expr) -> asm::Expr {
    asm::Expr::Let(dest, val, Box::new(body))
}

fn reg(r: asm::Reg) -> asm::Val {
    asm::Val::Reg(r)
}

fn global(idx: u8) -> asm::Val {
    asm::Val::Global(idx)
}

fn capture(idx: u8) -> asm::Val {
    asm::Val::Capture(idx)
}

fn ctor(tag: u8, fields: Vec<asm::Reg>) -> asm::Val {
    asm::Val::Ctor(tag, fields)
}

fn field(r: asm::Reg, idx: u8) -> asm::Val {
    asm::Val::Field(r, idx)
}

fn cont_lam(captures: Vec<asm::Reg>, body: asm::Expr) -> asm::Val {
    asm::Val::ContLam(asm::ContLam { captures, body: Box::new(body) })
}

fn encore(f: asm::Reg, arg: asm::Reg, k: asm::Reg) -> asm::Expr {
    asm::Expr::Encore(f, arg, k)
}

fn letrec(dest: asm::Reg, captures: Vec<asm::Reg>, body: asm::Expr, rest: asm::Expr) -> asm::Expr {
    asm::Expr::Letrec(dest, asm::Fun { captures, body: Box::new(body) }, Box::new(rest))
}

fn match_(scrutinee: asm::Reg, base: u8, cases: Vec<(u8, asm::Reg, asm::Expr)>) -> asm::Expr {
    asm::Expr::Match(
        scrutinee, base,
        cases.into_iter().map(|(arity, unpack_base, body)| asm::Case { arity, unpack_base, body }).collect(),
    )
}

use asm::{SELF, A1, CONT, X01, NULL};

fn x(n: u8) -> asm::Reg { X01 + n - 1 }

// -- Fin / Global --

#[test]
fn test_halt_global() {
    // define("main", Fin("main")) -> main is global 0, loaded into X01
    let m = module(vec![
        define("main", cps::Expr::Fin("main".into())),
    ]);
    assert_eq!(resolve_one(&m), let_(x(1), global(0), fin(x(1))));
}

// -- Let with Var --

#[test]
fn test_let_var() {
    // g is global 0, main is global 1
    // main body: Let("x", Var("g"), Fin("x"))
    // free = {"g"}, load g into X01, then Let binds x to X02
    let m = module(vec![
        define("g", cps::Expr::Fin("g".into())),
        define("main", cps::Expr::Let(
            "x".into(),
            cps::Val::Var("g".into()),
            Box::new(cps::Expr::Fin("x".into())),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(x(1), global(0), let_(x(2), reg(x(1)), fin(x(2))))
    );
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
    assert_eq!(resolve_one(&m), let_(x(1), ctor(5, vec![]), fin(x(1))));
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
        let_(x(1), ctor(1, vec![]),
            let_(x(2), ctor(2, vec![]),
                let_(x(3), ctor(0, vec![x(1), x(2)]),
                    fin(x(3)))))
    );
}

// -- Multiple lets, reference order --

#[test]
fn test_multiple_lets_ref_first() {
    // g0=global(0), g1=global(1), main=global(2)
    // free of main body = {"g0", "g1"}, loaded into X01=g0, X02=g1
    // Let("x", Var("g0"), ...) → x gets X03, val=Reg(X01)
    // Let("y", Var("g1"), ...) → y gets X04, val=Reg(X02)
    // Fin("x") → Fin(X03)
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
        let_(x(1), global(0),
            let_(x(2), global(1),
                let_(x(3), reg(x(1)),
                    let_(x(4), reg(x(2)),
                        fin(x(3))))))
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
        let_(x(1), global(0),
            let_(x(2), global(1),
                let_(x(3), reg(x(1)),
                    let_(x(4), reg(x(2)),
                        fin(x(4))))))
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
        let_(x(1), ctor(1, vec![]),
            let_(x(2), ctor(2, vec![]),
                let_(x(3), ctor(0, vec![x(1), x(2)]),
                    let_(x(4), field(x(3), 1),
                        fin(x(4))))))
    );
}

// -- Cont: identity continuation --

#[test]
fn test_cont_identity() {
    // define("main", Let("f", Cont(x -> Fin(x)), Let("_nc", NullCont, Encore("f", "main", "_nc"))))
    // globals = {"main": 0}
    // free = {"main"} → load global 0 into X01
    // Let("f", Cont(...)): cont body Fin("x") has free={}, captures=[], no globals used inside
    //   → ContLam { captures: [], body: Let(X01, Reg(A1), Fin(X01)) }
    //   bind "f" → X02
    // NullCont: bind "_nc" → 0xFF (no Let emitted)
    // Encore("f", "main", "_nc") → Encore(X02, X01, NULL)
    let m = module(vec![
        define("main", cps::Expr::Let(
            "f".into(),
            cps::Val::Cont(cps::Cont {
                param: "x".into(),
                body: Box::new(cps::Expr::Fin("x".into())),
            }),
            Box::new(cps::Expr::Let(
                "_nc".into(), cps::Val::NullCont,
                Box::new(cps::Expr::Encore("f".into(), "main".into(), "_nc".into())),
            )),
        )),
    ]);
    assert_eq!(
        resolve_one(&m),
        let_(x(1), global(0),
            let_(x(2), cont_lam(vec![], let_(x(1), reg(A1), fin(x(1)))),
                encore(x(2), x(1), NULL)))
    );
}

// -- Cont: global accessed inside cont (loaded via GLOBAL) --

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
            Box::new(cps::Expr::Let(
                "_nc".into(), cps::Val::NullCont,
                Box::new(cps::Expr::Encore("f".into(), "main".into(), "_nc".into())),
            )),
        )),
    ]);
    // globals = {"g": 0, "main": 1}
    // outer free = {"g", "main"} → X01=global(0), X02=global(1)
    // cont body: Fin("g"). free={"g"}. g is a global → used_globals=[(g,0)], captures=[]
    //   inner: bind_local("x")→X01. bind_local("g")→X02. resolve Fin("g")→Fin(X02).
    //   wrap globals: Let(X02, Global(0), Fin(X02))
    //   wrap arg: Let(X01, Reg(A1), ...)
    //   ContLam { captures: [], body: Let(X01, Reg(A1), Let(X02, Global(0), Fin(X02))) }
    // bind "f" → X03
    // NullCont: bind "_nc" → NULL
    // Encore("f", "main", "_nc") → Encore(X03, X02, NULL)
    assert_eq!(
        resolve_one(&m),
        let_(x(1), global(0),
            let_(x(2), global(1),
                let_(x(3), cont_lam(vec![],
                        let_(x(1), reg(A1), let_(x(2), global(0), fin(x(2))))),
                    encore(x(3), x(2), NULL))))
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
                Box::new(cps::Expr::Let(
                    "_nc".into(), cps::Val::NullCont,
                    Box::new(cps::Expr::Encore("f".into(), "g1".into(), "_nc".into())),
                )),
            )),
        )),
    ]);
    // globals = {"g0":0, "g1":1, "main":2}
    // outer free = {"g0", "g1"} → X01=global(0), X02=global(1)
    // Let("v", Var("g0")) → val=Reg(X01), bind "v"→X03
    // cont body: Fin("v"). free={"v"}. v is not a global → capture. captures=["v"]
    //   outer lookup("v")=X03. captures=[X03]
    //   inner: bind_local("x")→X01. bind_local("v")→X02. capture_regs=[(X02,0)]
    //   Fin("v")→Fin(X02). wrap capture: Let(X02, Capture(0), Fin(X02))
    //   wrap arg: Let(X01, Reg(A1), ...)
    //   ContLam { captures: [X03], body: Let(X01, Reg(A1), Let(X02, Capture(0), Fin(X02))) }
    // bind "f" → X04
    // NullCont → 0xFF
    // Encore("f","g1","_nc") → Encore(X04, X02, NULL)
    assert_eq!(
        resolve_one(&m),
        let_(x(1), global(0),
            let_(x(2), global(1),
                let_(x(3), reg(x(1)),
                    let_(x(4), cont_lam(vec![x(3)],
                            let_(x(1), reg(A1), let_(x(2), capture(0), fin(x(2))))),
                        encore(x(4), x(2), NULL)))))
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
                    Box::new(cps::Expr::Let(
                        "_nc".into(), cps::Val::NullCont,
                        Box::new(cps::Expr::Encore("f".into(), "main".into(), "_nc".into())),
                    )),
                )),
            )),
        )),
    ]);
    // globals = {"main": 0}. free={"main"}→X01=global(0).
    // Let("a", Ctor(1,[])): X02. Let("b", Ctor(2,[])): X03.
    // cont body: Ctor(0, ["a","b"]), Fin("pair"). free={"a","b"}. captures=["a","b"].
    //   outer: lookup("a")=X02, lookup("b")=X03. captures=[X02, X03].
    //   inner: bind_local("x")→X01. bind_local("a")→X02, bind_local("b")→X03.
    //   Ctor(0,["a","b"])→Ctor(0,[X02,X03]). bind "pair"→X04. Fin→Fin(X04).
    //   wrap captures, wrap arg
    // bind "f" → X04
    // NullCont → 0xFF
    // Encore("f","main","_nc") → Encore(X04, X01, NULL)
    assert_eq!(
        resolve_one(&m),
        let_(x(1), global(0),
            let_(x(2), ctor(1, vec![]),
                let_(x(3), ctor(2, vec![]),
                    let_(x(4), cont_lam(vec![x(2), x(3)],
                            let_(x(1), reg(A1),
                                let_(x(2), capture(0),
                                    let_(x(3), capture(1),
                                        let_(x(4), ctor(0, vec![x(2), x(3)]),
                                            fin(x(4))))))),
                        encore(x(4), x(1), NULL)))))
    );
}

// -- Match: nullary branches --

#[test]
fn test_match_branches() {
    // Three globals, last one does a match
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
    // globals: g0=0,g1=1,g2=2,main=3. free={"g0","g1","g2"}
    // X01=global(0), X02=global(1), X03=global(2)
    // Let("c", Var("g0")): val=Reg(X01), bind "c"→X04
    // Match("c"): scrutinee=X04
    //   case0: no binds. fin("g1")→fin(X02). unpack_base=X05 (local_count=4 before case, 4+3=7)
    //   case1: no binds. fin("g2")→fin(X03). unpack_base=X05
    assert_eq!(
        resolve_one(&m),
        let_(x(1), global(0),
            let_(x(2), global(1),
                let_(x(3), global(2),
                    let_(x(4), reg(x(1)),
                        match_(x(4), 0, vec![
                            (0, x(5), fin(x(2))),
                            (0, x(5), fin(x(3))),
                        ])))))
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
    // No globals referenced. X01=a, X02=b, X03=pair.
    // Match case: binds fst,snd. local_count=3 before case. fst→X04, snd→X05.
    // unpack_base = X01 + 3 = x(4)
    // Fin("snd")→Fin(x(5))
    assert_eq!(
        resolve_one(&m),
        let_(x(1), ctor(1, vec![]),
            let_(x(2), ctor(2, vec![]),
                let_(x(3), ctor(0, vec![x(1), x(2)]),
                    match_(x(3), 0, vec![
                        (2, x(4), fin(x(5))),
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
        let_(x(1), ctor(1, vec![]),
            let_(x(2), ctor(2, vec![]),
                let_(x(3), ctor(0, vec![x(1), x(2)]),
                    match_(x(3), 0, vec![
                        (2, x(4), fin(x(4))),
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
                body: Box::new(cps::Expr::Let(
                    "_nc".into(), cps::Val::NullCont,
                    Box::new(cps::Expr::Encore("k".into(), "x".into(), "_nc".into())),
                )),
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
    // globals = {"main": 0}
    // outer free = {"main"} (from Encore's "main" arg) → X01=global(0)
    // Letrec("f", fun { arg:"x", cont:"k", body: NullCont then Encore(k, x, _nc) })
    //   fun free vars: body references k, x, _nc. k and x are fun params (removed). _nc bound by Let.
    //     free = {} after removing x, k, f. No captures, no globals.
    //   inner: bind k=CONT, f=SELF, bind_local x→X01.
    //   NullCont: bind "_nc"→NULL. Encore(k, x, _nc)→Encore(CONT, X01, NULL).
    //   Fun { captures: [], body: Let(X01, Reg(A1), Encore(CONT, X01, NULL)) }
    // bind "f" → X02
    // Let("k0", Cont({param:"r", body:Fin("r")})):
    //   cont: bind_local r→X01. body: Fin(X01). wrap: Let(X01, Reg(A1), Fin(X01))
    //   bind "k0" → X03
    // Encore("f", "main", "k0") → Encore(X02, X01, X03)
    assert_eq!(
        resolve_one(&m),
        let_(x(1), global(0),
            letrec(x(2), vec![], let_(x(1), reg(A1), encore(CONT, x(1), NULL)),
                let_(x(3), cont_lam(vec![], let_(x(1), reg(A1), fin(x(1)))),
                    encore(x(2), x(1), x(3)))))
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
                                    body: cps::Expr::Let(
                                        "_nc".into(), cps::Val::NullCont,
                                        Box::new(cps::Expr::Encore("k".into(), "n".into(), "_nc".into())),
                                    ),
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
    // No globals referenced in body (no "main" use). free = {}.
    // zero→X01, s1→X02, s2→X03.
    // fun body: bind_local n→X01. Match(n, 0, cases). n=X01.
    //   case0: no binds. NullCont→NULL. Encore(k, n, _nc)→Encore(CONT, X01, NULL).
    //     unpack_base = X02 (local_count=1 inside fun, from n)
    //   case1: binds=["pred"]. bind pred→X02. Encore(f, pred, k)→Encore(SELF, X02, CONT).
    //     unpack_base = X02
    // bind "f" → X04.
    // k0: cont: bind_local r→X01. body: Let(X01, Reg(A1), Fin(X01)).
    // bind "k0" → X05.
    // Encore("f", "s2", "k0") → Encore(X04, X03, X05).
    assert_eq!(
        resolve_one(&m),
        let_(x(1), ctor(0, vec![]),
            let_(x(2), ctor(1, vec![x(1)]),
                let_(x(3), ctor(1, vec![x(2)]),
                    letrec(x(4), vec![],
                        let_(x(1), reg(A1),
                            match_(x(1), 0, vec![
                                (0, x(2), encore(CONT, x(1), NULL)),
                                (1, x(2), encore(SELF, x(2), CONT)),
                            ])),
                        let_(x(5), cont_lam(vec![], let_(x(1), reg(A1), fin(x(1)))),
                            encore(x(4), x(3), x(5)))))))
    );
}

