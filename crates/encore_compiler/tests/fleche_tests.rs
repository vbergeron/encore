use encore_compiler::pipeline;
use encore_fleche;
use encore_vm::program::Program;
use encore_vm::value::Value;
use encore_vm::vm::Vm;

fn run(source: &str) -> Value {
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module);
    let prog = Program::parse(&binary).unwrap();
    let mut mem = [Value::from_u32(0); 4096];
    let globals = vec![Value::from_u32(0); prog.n_globals() as usize];
    let mut vm = Vm::new(prog.code, prog.arity_table, &globals, &mut mem);
    vm.run().unwrap()
}

// -- Nullary ctor --

#[test]
fn test_nullary_ctor() {
    let result = run("
        data Zero | Succ(n)
        define main as Zero
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Let + var --

#[test]
fn test_let_var() {
    let result = run("
        data True | False
        define main as let x = True in x
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Identity function --

#[test]
fn test_identity() {
    let result = run("
        data True | False
        define main as let id = x -> x in id True
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Nested app --

#[test]
fn test_nested_app() {
    let result = run("
        data True | False
        define main as let id = x -> x in id (id True)
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Ctor with fields --

#[test]
fn test_ctor_with_fields() {
    let result = run("
        data True | False
        data Pair(a, b)
        define main as Pair(True, False)
    ");
    assert_eq!(result.ctor_tag(), 2);
}

// -- Field access --

#[test]
fn test_field_access() {
    let result = run("
        data True | False
        data Pair(a, b)
        define main as field 1 of Pair(True, False)
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Match branches --

#[test]
fn test_match_branch0() {
    let result = run("
        data True | False
        define main as
          match True
            case True -> False
            case False -> True
          end
    ");
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_match_branch1() {
    let result = run("
        data True | False
        define main as
          match False
            case True -> False
            case False -> True
          end
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Match with binds --

#[test]
fn test_match_with_binds() {
    let result = run("
        data True | False
        data Pair(a, b)
        define main as
          match Pair(True, False)
            case Pair(x, y) -> y
          end
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Peano countdown --

#[test]
fn test_peano_countdown() {
    let result = run("
        data Zero | Succ(n)
        define main as
          fix countdown n =
            match n
              case Zero -> n
              case Succ(pred) -> countdown pred
            end
          in countdown Succ(Succ(Succ(Zero)))
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Lambda capture --

#[test]
fn test_lambda_capture() {
    let result = run("
        data True | False
        define main as
          let v = True in
          let f = x -> v in
          f False
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Constant function --

#[test]
fn test_constant_fn() {
    let result = run("
        data A | B | C
        define main as
          let k = x -> y -> x in
          k A B
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Multi-data declarations --

#[test]
fn test_multi_data() {
    let result = run("
        data Zero | Succ(n)
        data True | False
        define main as True
    ");
    assert_eq!(result.ctor_tag(), 2);
}

// -- Optional leading pipe in data --

#[test]
fn test_leading_pipe() {
    let result = run("
        data
          | Zero
          | Succ(n)
        define main as Succ(Zero)
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Triple nested app --

#[test]
fn test_triple_nested_app() {
    let result = run("
        data X
        define main as
          let id = x -> x in
          id (id (id X))
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Field of nested ctor --

#[test]
fn test_field_first() {
    let result = run("
        data A | B
        data Pair(x, y)
        define main as field 0 of Pair(A, B)
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Fix with match returning ctor --

#[test]
fn test_fix_map() {
    let result = run("
        data Zero | Succ(n)
        data True | False
        define main as
          fix is_zero n =
            match n
              case Zero -> True
              case Succ(p) -> False
            end
          in is_zero Zero
    ");
    assert_eq!(result.ctor_tag(), 2);
}

// -- Integer literal --

#[test]
fn test_int_literal() {
    let result = run("define main as 42");
    assert!(result.is_int());
    assert_eq!(result.int_value(), 42);
}

// -- Builtin add --

#[test]
fn test_builtin_add() {
    let result = run("define main as builtin add 3 4");
    assert!(result.is_int());
    assert_eq!(result.int_value(), 7);
}

// -- Builtin sub --

#[test]
fn test_builtin_sub() {
    let result = run("define main as builtin sub 10 3");
    assert!(result.is_int());
    assert_eq!(result.int_value(), 7);
}

// -- Builtin mul --

#[test]
fn test_builtin_mul() {
    let result = run("define main as builtin mul 6 7");
    assert!(result.is_int());
    assert_eq!(result.int_value(), 42);
}

// -- Builtin eq true --

#[test]
fn test_builtin_eq_true() {
    let result = run("define main as builtin eq 3 3");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- Builtin eq false --

#[test]
fn test_builtin_eq_false() {
    let result = run("define main as builtin eq 3 4");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Builtin lt true --

#[test]
fn test_builtin_lt_true() {
    let result = run("define main as builtin lt 3 5");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- Builtin lt false --

#[test]
fn test_builtin_lt_false() {
    let result = run("define main as builtin lt 5 3");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Integer as ctor field --

#[test]
fn test_int_in_ctor_field() {
    let result = run("
        data Pair(a, b)
        define main as field 0 of Pair(42, 0)
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value(), 42);
}

// -- Builtin lt with match --

#[test]
fn test_builtin_lt_with_match() {
    let result = run("
        data False | True
        define main as
          let r = builtin lt 3 5 in
          match r
            case False -> 0
            case True -> 1
          end
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value(), 1);
}

// -- Arithmetic with let bindings --

#[test]
fn test_arithmetic_let() {
    let result = run("
        define main as
          let x = 10 in
          let y = 20 in
          builtin add x y
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value(), 30);
}
