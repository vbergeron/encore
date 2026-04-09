use encore_compiler::ir::ds;
use encore_compiler::pass::{cps_transform, emit::Emitter, resolver};
use encore_vm::program::Program;
use encore_vm::value::{HeapAddress, Value};
use encore_vm::vm::Vm;

fn ctor(tag: u8) -> Value {
    Value::ctor(tag, HeapAddress::NULL)
}

fn run_ds(module: ds::Module, define_idx: usize, globals: &[Value]) -> Value {
    let cps_module = cps_transform::transform_module(module);
    let ir_module = resolver::resolve_module(&cps_module);
    let define = &ir_module.defines[define_idx];
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&define.body);
    let binary = emitter.serialize(globals.len() as u16);
    let prog = Program::parse(&binary).unwrap();
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::new(prog.code, prog.arity_table, globals, &mut mem);
    vm.run().unwrap()
}

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

// -- Trivial: just return a global --

#[test]
fn test_var() {
    let m = module(vec![
        define("main", var("main")),
    ]);
    let result = run_ds(m, 0, &[ctor(42)]);
    assert_eq!(result.ctor_tag(), 42);
}

// -- Let + Var --

#[test]
fn test_let_var() {
    // let x = main in x
    let m = module(vec![
        define("main", ds_let("x", var("main"), var("x"))),
    ]);
    let result = run_ds(m, 0, &[ctor(7)]);
    assert_eq!(result.ctor_tag(), 7);
}

// -- Ctor --

#[test]
fn test_ctor_nullary() {
    // let c = Ctor(5, []) in c
    let m = module(vec![
        define("main", ds_let("c", ds_ctor(5, vec![]), var("c"))),
    ]);
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 5);
}

#[test]
fn test_ctor_nested() {
    // Ctor(0, [Ctor(1, []), Ctor(2, [])])
    let m = module(vec![
        define("main", ds_ctor(0, vec![ds_ctor(1, vec![]), ds_ctor(2, vec![])])),
    ]);
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 0);
}

// -- Field --

#[test]
fn test_field_of_ctor() {
    // field(Ctor(0, [Ctor(7, [])]), 0)
    let m = module(vec![
        define("main", field(ds_ctor(0, vec![ds_ctor(7, vec![])]), 0)),
    ]);
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 7);
}

#[test]
fn test_field_second() {
    // field(Ctor(0, [Ctor(1, []), Ctor(2, [])]), 1)
    let m = module(vec![
        define("main", field(ds_ctor(0, vec![ds_ctor(1, vec![]), ds_ctor(2, vec![])]), 1)),
    ]);
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 2);
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
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 42);
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
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 10);
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
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 33);
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
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 15);
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
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 10);
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
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 20);
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
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 2);
}

// -- Letrec: Peano countdown --

#[test]
fn test_peano_countdown() {
    // letrec countdown n =
    //   match n base=0 {
    //     [] -> n,                       -- Zero
    //     [pred] -> countdown(pred)      -- Succ(pred)
    //   }
    // in countdown(Succ(Succ(Zero)))
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
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 0);
}

// -- App with ctor arg (non-trivial argument normalization) --

#[test]
fn test_app_ctor_arg() {
    // let fst = \pair -> field(pair, 0) in
    // fst(Ctor(0, [Ctor(5, []), Ctor(9, [])]))
    let m = module(vec![
        define("main", ds_let(
            "fst", lam("pair", field(var("pair"), 0)),
            app(var("fst"), ds_ctor(0, vec![ds_ctor(5, vec![]), ds_ctor(9, vec![])])),
        )),
    ]);
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 5);
}

// -- Triple nested app: f(f(f(x))) --

#[test]
fn test_triple_nested_app() {
    // let id = \x -> x in id(id(id(Ctor(77, []))))
    let m = module(vec![
        define("main", ds_let(
            "id", lam("x", var("x")),
            app(var("id"), app(var("id"), app(var("id"), ds_ctor(77, vec![])))),
        )),
    ]);
    let result = run_ds(m, 0, &[ctor(0)]);
    assert_eq!(result.ctor_tag(), 77);
}
