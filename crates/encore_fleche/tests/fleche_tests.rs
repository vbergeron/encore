use encore_compiler::pipeline;
use encore_fleche;
use encore_vm::error::ExternError;
use encore_vm::program::Program;
use encore_vm::value::{Value, GlobalAddress};
use encore_vm::vm::Vm;

fn run(source: &str) -> Value {
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    vm.global_raw(GlobalAddress::new(0))
}

fn run_multi(source: &str) -> Value {
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    vm.global_raw(GlobalAddress::new(last as u16))
}

// -- Nullary ctor --

#[test]
fn test_nullary_ctor() {
    let result = run("
        data Zero | Succ(n)
        let main = Zero
    ");
    assert_eq!(result.ctor_tag(), 5);
}

// -- Let + var --

#[test]
fn test_let_var() {
    let result = run("
        let main = let x = True in x
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Identity function --

#[test]
fn test_identity() {
    let result = run("
        let main = let id = x -> x in id True
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Nested app --

#[test]
fn test_nested_app() {
    let result = run("
        let main = let id = x -> x in id (id True)
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Ctor with fields --

#[test]
fn test_ctor_with_fields() {
    let result = run("
        data Pair(a, b)
        let main = Pair(True, False)
    ");
    assert_eq!(result.ctor_tag(), 4);
}

// -- Field access --

#[test]
fn test_field_access() {
    let result = run("
        data Pair(a, b)
        let main = field 1 of Pair(True, False)
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Match branches --

#[test]
fn test_match_branch0() {
    let result = run("
        let main =
          match True
            | True -> False
            | False -> True
          end
    ");
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_match_branch1() {
    let result = run("
        let main =
          match False
            | True -> False
            | False -> True
          end
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Match with binds --

#[test]
fn test_match_with_binds() {
    let result = run("
        data Pair(a, b)
        let main =
          match Pair(True, False)
            | Pair(x, y) -> y
          end
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Peano countdown --

#[test]
fn test_peano_countdown() {
    let result = run("
        data Zero | Succ(n)
        let main =
          let rec countdown n =
            match n
              | Zero -> n
              | Succ(pred) -> countdown pred
            end
          in countdown Succ(Succ(Succ(Zero)))
    ");
    assert_eq!(result.ctor_tag(), 5);
}

// -- Lambda capture --

#[test]
fn test_lambda_capture() {
    let result = run("
        let main =
          let v = True in
          let f = x -> v in
          f False
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Constant function --

#[test]
fn test_constant_fn() {
    let result = run("
        data A | B | C
        let main =
          let k = x -> y -> x in
          k A B
    ");
    assert_eq!(result.ctor_tag(), 5);
}

// -- Multi-data declarations --

#[test]
fn test_multi_data() {
    let result = run("
        data Zero | Succ(n)
        data True | False
        let main = True
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Optional leading pipe in data --

#[test]
fn test_leading_pipe() {
    let result = run("
        data
          | Zero
          | Succ(n)
        let main = Succ(Zero)
    ");
    assert_eq!(result.ctor_tag(), 6);
}

// -- Triple nested app --

#[test]
fn test_triple_nested_app() {
    let result = run("
        data X
        let main =
          let id = x -> x in
          id (id (id X))
    ");
    assert_eq!(result.ctor_tag(), 5);
}

// -- Field of nested ctor --

#[test]
fn test_field_first() {
    let result = run("
        data A | B
        data Pair(x, y)
        let main = field 0 of Pair(A, B)
    ");
    assert_eq!(result.ctor_tag(), 5);
}

// -- Fix with match returning ctor --

#[test]
fn test_fix_map() {
    let result = run("
        data Zero | Succ(n)
        let main =
          let rec is_zero n =
            match n
              | Zero -> True
              | Succ(p) -> False
            end
          in is_zero Zero
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Integer literal --

#[test]
fn test_int_literal() {
    let result = run("let main = 42");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

// -- Builtin add --

#[test]
fn test_builtin_add() {
    let result = run("let main = builtin add 3 4");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

// -- Builtin sub --

#[test]
fn test_builtin_sub() {
    let result = run("let main = builtin sub 10 3");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

// -- Builtin mul --

#[test]
fn test_builtin_mul() {
    let result = run("let main = builtin mul 6 7");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

// -- Builtin eq true --

#[test]
fn test_builtin_eq_true() {
    let result = run("let main = builtin eq 3 3");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- Builtin eq false --

#[test]
fn test_builtin_eq_false() {
    let result = run("let main = builtin eq 3 4");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Builtin lt true --

#[test]
fn test_builtin_lt_true() {
    let result = run("let main = builtin lt 3 5");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- Builtin lt false --

#[test]
fn test_builtin_lt_false() {
    let result = run("let main = builtin lt 5 3");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Integer as ctor field --

#[test]
fn test_int_in_ctor_field() {
    let result = run("
        data Pair(a, b)
        let main = field 0 of Pair(42, 0)
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

// -- Builtin lt with match --

#[test]
fn test_builtin_lt_with_match() {
    let result = run("
        let main =
          let r = builtin lt 3 5 in
          match r
            | False -> 0
            | True -> 1
          end
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 1);
}

// -- Arithmetic with let bindings --

#[test]
fn test_arithmetic_let() {
    let result = run("
        let main =
          let x = 10 in
          let y = 20 in
          builtin add x y
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 30);
}

// -- Multi-arg lambda partial application --

#[test]
fn test_multi_arg_lambda_partial_apply() {
    let result = run("
        data A | B
        let main =
          let f = x -> y -> match x
            | A -> y
            | B -> y
          end
          in
          let g = f A in
          g 42
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_exact_multi_arg_call() {
    let result = run("
        let main =
          let f = x -> y -> builtin add x y in
          f 3 4
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_partial_apply_define() {
    let result = run_multi("
        let add = x -> y -> builtin add x y
        let main =
          let inc = add 1 in
          inc 41
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_over_application() {
    let result = run("
        let main =
          let f = x -> y -> z -> builtin add y z in
          f 0 3 4
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_chained_partial_application() {
    let result = run("
        let main =
          let f = x -> y -> z -> builtin add (builtin add x y) z in
          let g = f 1 in
          let h = g 2 in
          h 3
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 6);
}

#[test]
fn test_higher_order_unknown_callee() {
    let result = run("
        let main =
          let apply = f -> x -> f x in
          apply (y -> builtin add y 1) 41
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_letrec_as_value() {
    let result = run("
        let main =
          let rec f x = builtin add x 1 in
          let g = f in
          g 41
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

// -- Extern / FFI --

fn run_with_externs(source: &str, externs: &[(u16, encore_vm::vm::ExternFn)]) -> Value {
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    for &(slot, f) in externs {
        vm.register_extern(slot, f);
    }
    vm.load(&prog).unwrap();
    vm.global_raw(GlobalAddress::new(last as u16))
}

#[test]
fn test_extern_basic() {
    fn triple(_vm: &mut Vm, v: Value) -> Result<Value, ExternError> {
        Ok(Value::int(v.int_value().unwrap() * 3))
    }

    let result = run_with_externs("
        let extern triple_it 0

        let main = triple_it 7
    ", &[(0, triple)]);
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 21);
}

#[test]
fn test_extern_composed() {
    fn double(_vm: &mut Vm, v: Value) -> Result<Value, ExternError> {
        Ok(Value::int(v.int_value().unwrap() * 2))
    }
    fn negate(_vm: &mut Vm, v: Value) -> Result<Value, ExternError> {
        Ok(Value::int(-v.int_value().unwrap()))
    }

    let result = run_with_externs("
        let extern dbl 0
        let extern neg 1

        let main = neg (dbl 5)
    ", &[(0, double), (1, negate)]);
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), -10);
}

#[test]
fn test_extern_with_wrapper() {
    fn raw_add(_vm: &mut Vm, v: Value) -> Result<Value, ExternError> {
        let a = v.ctor_tag();
        Ok(Value::int(a as i32 * 100))
    }

    let result = run_with_externs("
        let extern raw_add 0
        let main = raw_add True
    ", &[(0, raw_add)]);
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 100);
}

// -- ds_uncurry: immediate multi-arg lambda application --

#[test]
fn test_immediate_multi_arg_lambda() {
    let result = run("
        let main = (x -> y -> builtin add x y) 3 4
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_immediate_two_arg_lambda_with_ctor() {
    let result = run("
        data Pair(a, b)
        let main =
          let p = (x -> y -> Pair(x, y)) 10 20 in
          field 0 of p
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 10);
}

#[test]
fn test_over_applied_known_function() {
    let result = run_multi("
        let const = x -> y -> x
        let main = const 42 99
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_immediate_lambda_in_let() {
    let result = run("
        let main =
          let r = (x -> y -> x) 10 20 in
          r
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 10);
}

#[test]
fn test_partial_apply_known_multi_arg() {
    let result = run_multi("
        let f = x -> y -> builtin add x y
        let main =
          let g = f 10 in
          g 20
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 30);
}

// -- Bytes parsing: list_nat_of_bytes --

fn collect_int_list(vm: &Vm, nil_tag: u8, cons_tag: u8, mut val: Value) -> Vec<i32> {
    let mut out = Vec::new();
    while val.is_ctor() && val.ctor_tag() == cons_tag {
        let head = vm.ctor_field(val, 0);
        out.push(head.int_value().unwrap());
        val = vm.ctor_field(val, 1);
    }
    assert!(val.is_ctor() && val.ctor_tag() == nil_tag, "list not terminated by Nil");
    out
}

#[test]
fn test_list_nat_of_bytes_empty() {
    let result = run_multi("
        data Nil | Cons(h, t)

        let list_nat_of_bytes = buf ->
          let len = builtin bytes_len buf in
          let rec go i =
            let done = builtin eq i len in
            match done
              | True -> Nil
              | False ->
                let b = builtin bytes_get buf i in
                let next = builtin add i 1 in
                Cons(b, go next)
            end
          in go 0

        let main = list_nat_of_bytes \"\"
    ");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 2); // Nil
}

#[test]
fn test_list_nat_of_bytes_hello() {
    let source = "
        data Nil | Cons(h, t)

        let list_nat_of_bytes = buf ->
          let len = builtin bytes_len buf in
          let rec go i =
            let done = builtin eq i len in
            match done
              | True -> Nil
              | False ->
                let b = builtin bytes_get buf i in
                let next = builtin add i 1 in
                Cons(b, go next)
            end
          in go 0

        let main = list_nat_of_bytes \"hello\"
    ";
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let result = vm.global_raw(GlobalAddress::new(last as u16));
    let bytes = collect_int_list(&vm, 2, 3, result);
    assert_eq!(bytes, vec![104, 101, 108, 108, 111]); // h e l l o
}

#[test]
fn test_list_nat_of_bytes_with_extern() {
    fn provide_bytes(vm: &mut Vm, _arg: Value) -> Result<Value, ExternError> {
        vm.alloc_bytes(b"AB").map_err(|_| ExternError::Custom("alloc failed"))
    }

    let source = "
        data Nil | Cons(h, t)

        let extern get_buf 0

        let list_nat_of_bytes = buf ->
          let len = builtin bytes_len buf in
          let rec go i =
            let done = builtin eq i len in
            match done
              | True -> Nil
              | False ->
                let b = builtin bytes_get buf i in
                let next = builtin add i 1 in
                Cons(b, go next)
            end
          in go 0

        let main = list_nat_of_bytes (get_buf 0)
    ";
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    vm.register_extern(0, provide_bytes);
    vm.load(&prog).unwrap();
    let result = vm.global_raw(GlobalAddress::new(last as u16));
    let bytes = collect_int_list(&vm, 2, 3, result);
    assert_eq!(bytes, vec![65, 66]); // A B
}

#[test]
fn test_compilation_deterministic() {
    let source = "
        data Red | Green | Blue
        data None | Some(x)
        let classify = color ->
          match color
            | Red -> Some(Red)
            | Green -> None
            | Blue -> Some(Blue)
          end
        let main = classify Green
    ";
    let compile = || {
        let (module, ctor_names) = encore_fleche::parse_with_metadata(source);
        let metadata = encore_compiler::pass::asm_emit::Metadata {
            ctor_names,
            global_names: module.defines.iter()
                .enumerate()
                .map(|(i, d)| (i as u8, d.name.clone()))
                .collect(),
        };
        pipeline::compile_module(module, None, Some(&metadata))
    };
    let reference = compile();
    for _ in 0..20 {
        assert_eq!(compile(), reference, "compilation produced different bytecode across runs");
    }
}

// -- Chained plain let --

#[test]
fn test_let_chain_plain_two() {
    let result = run("
        let main =
          let x = 10, y = 20 in
          builtin add x y
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 30);
}

#[test]
fn test_let_chain_plain_three() {
    let result = run("
        let main =
          let x = 1, y = 2, z = 3 in
          builtin add x (builtin add y z)
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 6);
}

#[test]
fn test_let_chain_plain_dependent() {
    let result = run("
        let main =
          let x = 10, y = builtin add x 5 in
          y
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 15);
}

// -- Chained let destructuring --

#[test]
fn test_let_destruct_single() {
    let result = run("
        data Pair(a, b)
        let main =
          let Pair(x, y) = Pair(10, 32) in
          builtin add x y
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_let_destruct_chain() {
    let result = run("
        data Pair(a, b)
        let main =
          let Pair(x, y) = Pair(3, 4),
              Pair(p, q) = Pair(x, y)
          in builtin add p q
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_let_destruct_chain_three() {
    let result = run("
        data Triple(a, b, c)
        data Pair(x, y)
        let main =
          let Triple(a, b, c) = Triple(1, 2, 3),
              Pair(p, q) = Pair(a, b),
              Pair(r, s) = Pair(q, c)
          in builtin add (builtin add p r) s
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 6);
}

#[test]
fn test_let_destruct_nullary() {
    let result = run("
        data Wrap(x)
        let main =
          let Wrap(v) = Wrap(42) in v
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

// -- if pattern binding --

#[test]
fn test_if_binding_success() {
    let result = run("
        data Ok(x) | Err
        let main =
          if Ok(v) = Ok(42)
          then v
          else 0
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_if_binding_failure() {
    let result = run("
        data Ok(x) | Err
        let main =
          if Ok(v) = Err
          then v
          else 99
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 99);
}

#[test]
fn test_if_binding_three_ctors() {
    let result = run("
        data Red | Green | Blue
        let main =
          if Green = Green
          then 1
          else 0
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 1);
}

#[test]
fn test_if_binding_three_ctors_miss() {
    let result = run("
        data Red | Green | Blue
        let main =
          if Green = Blue
          then 1
          else 0
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 0);
}

#[test]
fn test_if_binding_chain_both_succeed() {
    let result = run("
        data Ok(x) | Err
        data Some(x) | None
        let main =
          if Ok(a) = Ok(1),
             Some(b) = Some(2)
          then builtin add a b
          else 0
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 3);
}

#[test]
fn test_if_binding_chain_first_fails() {
    let result = run("
        data Ok(x) | Err
        data Some(x) | None
        let main =
          if Ok(a) = Err,
             Some(b) = Some(2)
          then builtin add a b
          else 99
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 99);
}

#[test]
fn test_if_binding_chain_second_fails() {
    let result = run("
        data Ok(x) | Err
        data Some(x) | None
        let main =
          if Ok(a) = Ok(1),
             Some(b) = None
          then builtin add a b
          else 99
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 99);
}

#[test]
fn test_if_binding_uses_outer_bind_in_chain() {
    let result = run("
        data Ok(x) | Err
        let main =
          if Ok(a) = Ok(10),
             Ok(b) = Ok(a)
          then b
          else 0
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 10);
}

// -- Wildcard match --

#[test]
fn test_match_wildcard_only_branch() {
    let result = run("
        data A | B | C
        let main =
          match A
            | A -> 1
            | _ -> 0
          end
    ");
    assert_eq!(result.int_value().unwrap(), 1);
}

#[test]
fn test_match_wildcard_fallthrough() {
    let result = run("
        data A | B | C
        let main =
          match C
            | A -> 1
            | _ -> 99
          end
    ");
    assert_eq!(result.int_value().unwrap(), 99);
}

#[test]
fn test_match_wildcard_middle_gap() {
    let result = run("
        data A | B | C | D
        let main =
          match B
            | A -> 1
            | D -> 4
            | _ -> 0
          end
    ");
    assert_eq!(result.int_value().unwrap(), 0);
}

#[test]
fn test_match_wildcard_with_fields() {
    let result = run("
        data Leaf | Node(l, r)
        let main =
          match Leaf
            | Node(l, r) -> builtin add l r
            | _ -> 42
          end
    ");
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_match_wildcard_preserves_explicit_binds() {
    let result = run("
        data Leaf | Node(l, r)
        let main =
          match Node(10, 20)
            | Node(l, r) -> builtin add l r
            | _ -> 0
          end
    ");
    assert_eq!(result.int_value().unwrap(), 30);
}

#[test]
fn test_match_wildcard_last_position() {
    let result = run("
        data Zero | Succ(n)
        let main =
          match Succ(Succ(Zero))
            | Zero -> 0
            | _ -> 1
          end
    ");
    assert_eq!(result.int_value().unwrap(), 1);
}

#[test]
fn test_match_wildcard_builtin_bool() {
    let result = run("
        let main =
          let r = builtin lt 3 5 in
          match r
            | True -> 1
            | _ -> 0
          end
    ");
    assert_eq!(result.int_value().unwrap(), 1);
}

// -- Wildcard match errors --

#[test]
fn test_match_wildcard_case_after_wildcard_is_error() {
    let mut parser = encore_fleche::parser::Parser::new("
        data A | B | C
        let main =
          match A
            | A -> 1
            | _ -> 0
            | B -> 2
          end
    ");
    match parser.parse_module() {
        Err(e) => assert!(e.message.contains("after wildcard"), "got: {}", e.message),
        Ok(_) => panic!("expected parse error"),
    }
}

#[test]
fn test_match_wildcard_duplicate_is_error() {
    let mut parser = encore_fleche::parser::Parser::new("
        data A | B | C
        let main =
          match A
            | A -> 1
            | _ -> 0
            | _ -> 2
          end
    ");
    match parser.parse_module() {
        Err(e) => assert!(e.message.contains("duplicate wildcard"), "got: {}", e.message),
        Ok(_) => panic!("expected parse error"),
    }
}

// -- Negative literals --

#[test]
fn test_negative_literal() {
    let result = run("
        let main = -7
    ");
    assert_eq!(result.int_value().unwrap(), -7);
}

#[test]
fn test_negative_literal_in_builtin() {
    let result = run("
        let main = builtin add 10 -3
    ");
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_negative_literal_in_ctor() {
    let result = run("
        data Wrap(val)
        let main = field 0 of Wrap(-42)
    ");
    assert_eq!(result.int_value().unwrap(), -42);
}

// -- String escapes --

#[test]
fn test_string_escape_newline() {
    let result = run(r#"
        let main = builtin bytes_len "hello\n"
    "#);
    assert_eq!(result.int_value().unwrap(), 6);
}

#[test]
fn test_string_escape_tab() {
    let result = run(r#"
        let main = builtin bytes_get "\t" 0
    "#);
    assert_eq!(result.int_value().unwrap(), 9);
}

#[test]
fn test_string_escape_null() {
    let result = run(r#"
        let main = builtin bytes_get "\0" 0
    "#);
    assert_eq!(result.int_value().unwrap(), 0);
}

#[test]
fn test_string_escape_backslash() {
    let result = run(r#"
        let main = builtin bytes_len "\\"
    "#);
    assert_eq!(result.int_value().unwrap(), 1);
}

#[test]
fn test_string_escape_quote() {
    let result = run(r#"
        let main = builtin bytes_len "\""
    "#);
    assert_eq!(result.int_value().unwrap(), 1);
}

#[test]
fn test_string_escape_mixed() {
    let result = run(r#"
        let main = builtin bytes_len "a\nb\tc"
    "#);
    assert_eq!(result.int_value().unwrap(), 5);
}

// -- Match exhaustiveness --

#[test]
fn test_match_non_exhaustive_missing_one() {
    let mut parser = encore_fleche::parser::Parser::new("
        data A | B | C
        let main =
          match A
          | A -> 1
          | B -> 2
          end
    ");
    match parser.parse_module() {
        Err(e) => {
            assert!(e.message.contains("non-exhaustive"), "got: {}", e.message);
            assert!(e.message.contains("C"), "error should name missing ctor C, got: {}", e.message);
        }
        Ok(_) => panic!("expected non-exhaustive match error"),
    }
}

#[test]
fn test_match_non_exhaustive_missing_multiple() {
    let mut parser = encore_fleche::parser::Parser::new("
        data A | B | C | D
        let main =
          match A
          | A -> 1
          end
    ");
    match parser.parse_module() {
        Err(e) => {
            assert!(e.message.contains("non-exhaustive"), "got: {}", e.message);
            assert!(e.message.contains("B"), "error should name missing ctor B, got: {}", e.message);
            assert!(e.message.contains("C"), "error should name missing ctor C, got: {}", e.message);
            assert!(e.message.contains("D"), "error should name missing ctor D, got: {}", e.message);
        }
        Ok(_) => panic!("expected non-exhaustive match error"),
    }
}

#[test]
fn test_match_cross_type_error() {
    let mut parser = encore_fleche::parser::Parser::new("
        data A | B
        data C | D
        let main =
          match A
          | A -> 1
          | C -> 2
          end
    ");
    match parser.parse_module() {
        Err(e) => assert!(e.message.contains("different types"), "got: {}", e.message),
        Ok(_) => panic!("expected cross-type match error"),
    }
}

#[test]
fn test_match_exhaustive_all_covered() {
    let result = run("
        data A | B | C
        let main =
          match B
          | A -> 1
          | B -> 2
          | C -> 3
          end
    ");
    assert_eq!(result.int_value().unwrap(), 2);
}
