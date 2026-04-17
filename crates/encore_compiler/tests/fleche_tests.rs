use encore_compiler::pipeline;
use encore_fleche;
use encore_vm::error::ExternError;
use encore_vm::program::Program;
use encore_vm::value::Value;
use encore_vm::vm::Vm;

fn run(source: &str) -> Value {
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    vm.global(0)
}

fn run_multi(source: &str) -> Value {
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    vm.global(last)
}

// -- Nullary ctor --

#[test]
fn test_nullary_ctor() {
    let result = run("
        data Zero | Succ(n)
        define main as Zero
    ");
    assert_eq!(result.ctor_tag(), 4);
}

// -- Let + var --

#[test]
fn test_let_var() {
    let result = run("
        define main as let x = True in x
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Identity function --

#[test]
fn test_identity() {
    let result = run("
        define main as let id = x -> x in id True
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Nested app --

#[test]
fn test_nested_app() {
    let result = run("
        define main as let id = x -> x in id (id True)
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Ctor with fields --

#[test]
fn test_ctor_with_fields() {
    let result = run("
        data Pair(a, b)
        define main as Pair(True, False)
    ");
    assert_eq!(result.ctor_tag(), 4);
}

// -- Field access --

#[test]
fn test_field_access() {
    let result = run("
        data Pair(a, b)
        define main as field 1 of Pair(True, False)
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Match branches --

#[test]
fn test_match_branch0() {
    let result = run("
        define main as
          match True
            case True -> False
            case False -> True
          end
    ");
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_match_branch1() {
    let result = run("
        define main as
          match False
            case True -> False
            case False -> True
          end
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Match with binds --

#[test]
fn test_match_with_binds() {
    let result = run("
        data Pair(a, b)
        define main as
          match Pair(True, False)
            case Pair(x, y) -> y
          end
    ");
    assert_eq!(result.ctor_tag(), 0);
}

// -- Peano countdown --

#[test]
fn test_peano_countdown() {
    let result = run("
        data Zero | Succ(n)
        define main as
          let rec countdown n =
            match n
              case Zero -> n
              case Succ(pred) -> countdown pred
            end
          in countdown Succ(Succ(Succ(Zero)))
    ");
    assert_eq!(result.ctor_tag(), 4);
}

// -- Lambda capture --

#[test]
fn test_lambda_capture() {
    let result = run("
        define main as
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
        define main as
          let k = x -> y -> x in
          k A B
    ");
    assert_eq!(result.ctor_tag(), 4);
}

// -- Multi-data declarations --

#[test]
fn test_multi_data() {
    let result = run("
        data Zero | Succ(n)
        data True | False
        define main as True
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
        define main as Succ(Zero)
    ");
    assert_eq!(result.ctor_tag(), 5);
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
    assert_eq!(result.ctor_tag(), 4);
}

// -- Field of nested ctor --

#[test]
fn test_field_first() {
    let result = run("
        data A | B
        data Pair(x, y)
        define main as field 0 of Pair(A, B)
    ");
    assert_eq!(result.ctor_tag(), 4);
}

// -- Fix with match returning ctor --

#[test]
fn test_fix_map() {
    let result = run("
        data Zero | Succ(n)
        define main as
          let rec is_zero n =
            match n
              case Zero -> True
              case Succ(p) -> False
            end
          in is_zero Zero
    ");
    assert_eq!(result.ctor_tag(), 1);
}

// -- Integer literal --

#[test]
fn test_int_literal() {
    let result = run("define main as 42");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

// -- Builtin add --

#[test]
fn test_builtin_add() {
    let result = run("define main as builtin add 3 4");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

// -- Builtin sub --

#[test]
fn test_builtin_sub() {
    let result = run("define main as builtin sub 10 3");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

// -- Builtin mul --

#[test]
fn test_builtin_mul() {
    let result = run("define main as builtin mul 6 7");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
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
    assert_eq!(result.int_value().unwrap(), 42);
}

// -- Builtin lt with match --

#[test]
fn test_builtin_lt_with_match() {
    let result = run("
        define main as
          let r = builtin lt 3 5 in
          match r
            case False -> 0
            case True -> 1
          end
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 1);
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
    assert_eq!(result.int_value().unwrap(), 30);
}

// -- Multi-arg lambda partial application --

#[test]
fn test_multi_arg_lambda_partial_apply() {
    let result = run("
        data A | B
        define main as
          let f = x -> y -> match x
            case A -> y
            case B -> y
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
        define main as
          let f = x -> y -> builtin add x y in
          f 3 4
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_partial_apply_define() {
    let result = run_multi("
        define add as x -> y -> builtin add x y
        define main as
          let inc = add 1 in
          inc 41
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_over_application() {
    let result = run("
        define main as
          let f = x -> y -> z -> builtin add y z in
          f 0 3 4
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_chained_partial_application() {
    let result = run("
        define main as
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
        define main as
          let apply = f -> x -> f x in
          apply (y -> builtin add y 1) 41
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_letrec_as_value() {
    let result = run("
        define main as
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
    vm.global(last)
}

#[test]
fn test_extern_basic() {
    fn triple(_vm: &mut Vm, v: Value) -> Result<Value, ExternError> {
        Ok(Value::int(v.int_value().unwrap() * 3))
    }

    let result = run_with_externs("
        define extern triple_it 0

        define main as triple_it 7
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
        define extern dbl 0
        define extern neg 1

        define main as neg (dbl 5)
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
        define extern raw_add 0
        define main as raw_add True
    ", &[(0, raw_add)]);
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 100);
}

// -- ds_uncurry: immediate multi-arg lambda application --

#[test]
fn test_immediate_multi_arg_lambda() {
    let result = run("
        define main as (x -> y -> builtin add x y) 3 4
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_immediate_two_arg_lambda_with_ctor() {
    let result = run("
        data Pair(a, b)
        define main as
          let p = (x -> y -> Pair(x, y)) 10 20 in
          field 0 of p
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 10);
}

#[test]
fn test_over_applied_known_function() {
    let result = run_multi("
        define const as x -> y -> x
        define main as const 42 99
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_immediate_lambda_in_let() {
    let result = run("
        define main as
          let r = (x -> y -> x) 10 20 in
          r
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 10);
}

#[test]
fn test_partial_apply_known_multi_arg() {
    let result = run_multi("
        define f as x -> y -> builtin add x y
        define main as
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

        define list_nat_of_bytes as buf ->
          let len = builtin bytes_len buf in
          let rec go i =
            let done = builtin eq i len in
            match done
              case True -> Nil
              case False ->
                let b = builtin bytes_get buf i in
                let next = builtin add i 1 in
                Cons(b, go next)
            end
          in go 0

        define main as list_nat_of_bytes \"\"
    ");
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 2); // Nil
}

#[test]
fn test_list_nat_of_bytes_hello() {
    let source = "
        data Nil | Cons(h, t)

        define list_nat_of_bytes as buf ->
          let len = builtin bytes_len buf in
          let rec go i =
            let done = builtin eq i len in
            match done
              case True -> Nil
              case False ->
                let b = builtin bytes_get buf i in
                let next = builtin add i 1 in
                Cons(b, go next)
            end
          in go 0

        define main as list_nat_of_bytes \"hello\"
    ";
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let result = vm.global(last);
    // Nil=2, Cons=3 (after False=0, True=1)
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

        define extern get_buf 0

        define list_nat_of_bytes as buf ->
          let len = builtin bytes_len buf in
          let rec go i =
            let done = builtin eq i len in
            match done
              case True -> Nil
              case False ->
                let b = builtin bytes_get buf i in
                let next = builtin add i 1 in
                Cons(b, go next)
            end
          in go 0

        define main as list_nat_of_bytes (get_buf 0)
    ";
    let module = encore_fleche::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 4096];
    let mut vm = Vm::init(&mut mem);
    vm.register_extern(0, provide_bytes);
    vm.load(&prog).unwrap();
    let result = vm.global(last);
    // Nil=2, Cons=3 (after False=0, True=1)
    let bytes = collect_int_list(&vm, 2, 3, result);
    assert_eq!(bytes, vec![65, 66]); // A B
}

#[test]
fn test_compilation_deterministic() {
    let source = "
        data Red | Green | Blue
        data None | Some(x)
        define classify as color ->
          match color
            case Red -> Some(Red)
            case Green -> None
            case Blue -> Some(Blue)
          end
        define main as classify Green
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

// -- Chained let destructuring --

#[test]
fn test_let_destruct_single() {
    let result = run("
        data Pair(a, b)
        define main as
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
        define main as
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
        define main as
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
        define main as
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
        define main as
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
        define main as
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
        define main as
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
        define main as
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
        define main as
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
        define main as
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
        define main as
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
        define main as
          if Ok(a) = Ok(10),
             Ok(b) = Ok(a)
          then b
          else 0
    ");
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 10);
}

// -- Scheme: fold_left with match+call init (inliner bug reproducer) --

fn run_scheme_int(source: &str) -> i32 {
    let module = encore_scheme::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 8192];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    vm.global(last).int_value().unwrap()
}

#[test]
fn test_scheme_fold_left_match_init() {
    // Minimal reproducer: 3-arg wrapper around fold_left where the
    // init accumulator is computed via match + function call.
    // Expected: id(42) = 42, then 42 + 1 = 43.
    let result = run_scheme_int("
        (define fold_left (lambdas (f l a0)
          (match l
             ((Nil) a0)
             ((Cons b l0) (@ fold_left f l0 (@ f a0 b))))))

        (define go (lambdas (ops n0 lst)
          (@ fold_left
            (lambdas (acc n) (+ acc n))
            lst
            (match ops
              ((Box h) (h n0))))))

        (define id (lambda (x) x))

        (define main (@ go `(Box ,id) 42 `(Cons ,1 ,`(Nil))))
    ");
    assert_eq!(result, 43);
}

// -- Scheme: bytes_of_list_nat --

fn run_scheme_bytes(source: &str) -> Vec<u8> {
    let module = encore_scheme::parse(source);
    let binary = pipeline::compile_module(module, None, None);
    let prog = Program::parse(&binary).unwrap();
    let last = prog.n_globals() - 1;
    let mut mem = [Value::from_u32(0); 8192];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let result = vm.global(last);
    assert!(result.is_bytes(), "expected bytes, got {}", result.type_name());
    let len = vm.bytes_len(result);
    (0..len).map(|i| vm.bytes_read(result, i)).collect()
}

const BYTES_OF_LIST_NAT_PRELUDE: &str = "
(define fold_left (lambdas (f l a0)
  (match l
     ((Nil) a0)
     ((Cons b l0) (@ fold_left f l0 (@ f a0 b))))))

(define bytes_of_list_nat (lambdas (bytesOps n0 lst)
  (@ fold_left (lambdas (acc n)
    (match bytesOps
       ((Build_BytesOps _ _ _ bytes_concat0)
         (@ bytes_concat0 acc
           (match bytesOps
              ((Build_BytesOps _ _ bytes_of_nat0 _) (bytes_of_nat0 n)))))))
    lst
    (match bytesOps
       ((Build_BytesOps _ _ bytes_of_nat0 _) (bytes_of_nat0 n0))))))

(define bytes_len (lambda (b) (bytes-len b)))
(define bytes_get (lambdas (b i) (bytes-get b i)))
(define bytes_of_nat (lambda (i) (int->byte i)))
(define bytes_concat (lambdas (l r) (bytes-concat l r)))
(define bytes_ops `(Build_BytesOps ,bytes_len ,bytes_get ,bytes_of_nat
  ,bytes_concat))
";

#[test]
fn test_scheme_bytes_of_list_nat_singleton() {
    let source = format!("{BYTES_OF_LIST_NAT_PRELUDE}
        (define main (@ bytes_of_list_nat bytes_ops 42 `(Nil)))
    ");
    assert_eq!(run_scheme_bytes(&source), vec![42]);
}

#[test]
fn test_scheme_bytes_of_list_nat_two_elements() {
    let source = format!("{BYTES_OF_LIST_NAT_PRELUDE}
        (define main (@ bytes_of_list_nat bytes_ops 1
          `(Cons ,2 ,`(Nil))))
    ");
    assert_eq!(run_scheme_bytes(&source), vec![1, 2]);
}

#[test]
fn test_scheme_bytes_of_list_nat_multi() {
    let source = format!("{BYTES_OF_LIST_NAT_PRELUDE}
        (define main (@ bytes_of_list_nat bytes_ops 1
          `(Cons ,2 ,`(Cons ,3 ,`(Nil)))))
    ");
    assert_eq!(run_scheme_bytes(&source), vec![1, 2, 3]);
}

#[test]
fn test_scheme_bytes_of_list_nat_boundary_values() {
    let source = format!("{BYTES_OF_LIST_NAT_PRELUDE}
        (define main (@ bytes_of_list_nat bytes_ops 0
          `(Cons ,127 ,`(Cons ,255 ,`(Nil)))))
    ");
    assert_eq!(run_scheme_bytes(&source), vec![0, 127, 255]);
}

