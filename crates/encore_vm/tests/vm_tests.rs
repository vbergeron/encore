use encore_vm::error::VmError;
use encore_vm::opcode::*;
use encore_vm::program::Program;
use encore_vm::value::{CodeAddress, HeapAddress, Value};
use encore_vm::vm::Vm;

fn run(code: &[u8], arity_table: &[u8]) -> Result<Value, VmError> {
    let prog = Program::new(code, arity_table, &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog)?;
    Ok(vm.global(0))
}

// -- Basic tests --

#[test]
fn test_pack_nullary() {
    let code = [PACK, 0, FIN];
    let arity_table = [0];
    let result = run(&code, &arity_table).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_pack_and_field() {
    let code = [
        PACK, 1,
        PACK, 2,
        PACK, 0,
        FIELD, 0,
        FIN,
    ];
    let arity_table = [2, 0, 0];
    let result = run(&code, &arity_table).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- Closure tests --

#[test]
fn test_closure_and_enter() {
    let code = [
        PACK, 0,                    // cont = ctor(0) (dummy)
        PACK, 0,                    // arg = ctor(0)
        FUNCTION, 9, 0, 1,          // function with code_ptr=9, sd=1
        ENCORE,                     // pop clo, pop arg, pop cont, enter
        // function body at byte 9:
        ARG,                        // push arg register
        FIN,
    ];
    let arity_table = [0];
    let result = run(&code, &arity_table).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_load_capture() {
    let code = [
        PACK, 0,                    // cont = ctor(0) (dummy)
        PACK, 0,                    // arg = ctor(0)
        PACK, 1,                    // value to capture = ctor(1)
        CLOSURE, 12, 0, 1, 1,      // closure with code_ptr=12, ncap=1, sd=1
        ENCORE,                     // pop clo, pop arg, pop cont, enter
        // closure body at byte 12:
        CAPTURE, 0,                 // push capture 0 = ctor(1)
        FIN,
    ];
    let arity_table = [0, 0];
    let result = run(&code, &arity_table).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_load_global() {
    let code = [
        // global 0 thunk: produce ctor(42)
        PACK, 42, FIN,
        // global 1 thunk: read global 0
        GLOBAL, 0, FIN,
    ];
    let arity_table = [0; 43];
    let prog = Program::new(&code, &arity_table, &[CodeAddress::new(0), CodeAddress::new(3)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let result = vm.global(1);
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 42);
}

// -- Match test --

#[test]
fn test_match() {
    let code = [
        PACK, 1,                    // ctor(1)
        MATCH, 0, 2,                // base=0, n=2
        9, 0,                       // off[0] = 9 (branch tag=0)
        12, 0,                      // off[1] = 12 (branch tag=1)
        // byte 9: branch tag=0
        PACK, 2,
        FIN,
        // byte 12: branch tag=1
        PACK, 3,
        FIN,
    ];
    let arity_table = [0, 0, 0, 0];
    let result = run(&code, &arity_table).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 3);
}

// -- SELF test (Peano countdown) --

#[test]
fn test_self_recursive() {
    let code = [
        PACK, 0,                    // dummy cont
        PACK, 0,                    // Zero
        PACK, 1,                    // Succ(Zero)
        PACK, 1,                    // Succ(Succ(Zero))  — arg
        FUNCTION, 13, 0, 3,         // function code_ptr=13, sd=3
        ENCORE,                     // pop clo, pop arg, pop cont, enter
        // countdown body at byte 13:
        ARG,                        // push arg
        MATCH, 0, 2,                // base=0, n=2
        21, 0,                      // off[0] = 21 (Zero branch)
        23, 0,                      // off[1] = 23 (Succ branch)
        // byte 21: Zero branch
        ARG,
        FIN,
        // byte 23: Succ branch
        CONT,                       // push cont (pass along)
        ARG,                        // push arg (the Succ ctor)
        FIELD, 0,                   // pop Succ(pred), push pred
        SELF,                       // push self
        ENCORE,                     // pop clo=self, pop arg=pred, pop cont, recurse
    ];
    let arity_table = [0, 1];
    let result = run(&code, &arity_table).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0); // Zero
}

// -- Integer tests --

#[test]
fn test_int_const() {
    let code = [INT, 42, 0, 0, FIN];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value(), 42);
}

#[test]
fn test_int_const_negative() {
    let code = [INT, 0xFF, 0xFF, 0xFF, FIN];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value(), -1);
}

#[test]
fn test_int_add() {
    let code = [
        INT, 3, 0, 0,
        INT, 4, 0, 0,
        INT_ADD,
        FIN,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value(), 7);
}

#[test]
fn test_int_sub() {
    let code = [
        INT, 10, 0, 0,
        INT, 3, 0, 0,
        INT_SUB,
        FIN,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value(), 7);
}

#[test]
fn test_int_mul() {
    let code = [
        INT, 6, 0, 0,
        INT, 7, 0, 0,
        INT_MUL,
        FIN,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value(), 42);
}

#[test]
fn test_int_eq_true() {
    let code = [INT, 5, 0, 0, INT, 5, 0, 0, INT_EQ, FIN];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_int_eq_false() {
    let code = [INT, 5, 0, 0, INT, 6, 0, 0, INT_EQ, FIN];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_int_lt_true() {
    let code = [INT, 3, 0, 0, INT, 5, 0, 0, INT_LT, FIN];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_int_lt_false() {
    let code = [INT, 5, 0, 0, INT, 3, 0, 0, INT_LT, FIN];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Error tests --

#[test]
fn test_heap_overflow() {
    let code = [PACK, 0];
    let arity_table = [100];
    let prog = Program::new(&code, &arity_table, &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 4];
    let mut vm = Vm::init(&mut mem);
    let result = vm.load(&prog);
    assert!(matches!(result, Err(VmError::HeapOverflow)));
}

#[test]
fn test_stack_overflow() {
    let code = [
        // global 0: produce int(0)
        INT_0, FIN,
        // global 1: push GLOBAL 0 five times -> overflow
        GLOBAL, 0,
        GLOBAL, 0,
        GLOBAL, 0,
        GLOBAL, 0,
        GLOBAL, 0,
    ];
    let prog = Program::with_sds(
        &code, &[],
        &[CodeAddress::new(0), CodeAddress::new(2)],
        &[1, 5],
    );
    let mut mem = [Value::from_u32(0); 4];
    let mut vm = Vm::init(&mut mem);
    let result = vm.load(&prog);
    assert!(matches!(result, Err(VmError::StackOverflow)));
}

#[test]
fn test_invalid_opcode() {
    let code = [0xFF];
    let result = run(&code, &[]);
    assert!(matches!(result, Err(VmError::InvalidOpcode(0xFF))));
}

// -- GC tests --

#[test]
fn test_gc_reclaims_dead_closures() {
    let code = [
        // global 0 thunk: produce function(@5)
        FUNCTION, 5, 0, 1, FIN,
        // function body at offset 5:
        ARG, FIN,
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 10];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let arg = Value::ctor(0, HeapAddress::NULL);
    for _ in 0..10 {
        let result = vm.call(0, arg).unwrap();
        assert!(result.is_ctor());
        assert_eq!(result.ctor_tag(), 0);
    }
}

#[test]
fn test_gc_preserves_live_data() {
    let code = [
        // main preamble:
        PACK, 0,                    // byte 0: dummy cont
        PACK, 0,                    // byte 2: arg = ctor(0)
        PACK, 1,                    // byte 4: dummy capture for garbage_func
        CLOSURE, 12, 0, 1, 3,      // byte 6: garbage_func at byte 12, ncap=1, sd=3. alloc 3. hp=3
        ENCORE,                     // byte 11: enter garbage_func

        // garbage_func body at byte 12:
        NULLADDR,                   // byte 12: push null cont for ENCORE
        ARG,                        // byte 13: push result
        PACK, 1,                    // byte 14: capture = ctor(1)
        CLOSURE, 22, 0, 1, 2,      // byte 16: real closure at byte 22, ncap=1, sd=2. alloc 3. hp=6
        ENCORE,                     // byte 21: ENCORE into real closure. garbage closure is now dead.

        // real_closure body at byte 22:
        ARG,                        // byte 22: push arg = ctor(0)
        PACK, 2,                    // byte 23: ctor(2, arity=1): pops ctor(0), alloc 2. GC!
        CAPTURE, 0,                 // byte 25: push capture 0 — should be ctor(1)
        FIN,                        // byte 27
    ];
    let arity_table = [0, 0, 1];
    let prog = Program::new(&code, &arity_table, &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 10];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let result = vm.global(0);
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- call() API test --

#[test]
fn test_call() {
    let code = [
        // global 0 thunk: produce function(@5)
        FUNCTION, 5, 0, 2, FIN,
        // function body at offset 5:
        ARG,                        // push arg
        MATCH, 0, 2,
        13, 0,                      // off[0] = 13 (Zero branch)
        15, 0,                      // off[1] = 15 (Succ branch)
        ARG,                        // byte 13: Zero -> push arg
        FIN,                        // byte 14
        ARG,                        // byte 15: Succ -> push arg
        FIELD, 0,                   // push pred
        FIN,
    ];
    let arity_table = [0, 1];
    let prog = Program::new(&code, &arity_table, &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();

    let arg = Value::ctor(0, HeapAddress::new(0));
    let result = vm.call(0, arg).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Extern tests --

#[test]
fn test_extern_dispatch() {
    fn double_it(v: Value) -> Value {
        Value::int(v.int_value() * 2)
    }

    let code = [
        // global 0: push 21, call extern 0, return result
        INT, 21, 0, 0,     // push int(21)
        EXTERN, 0, 0,      // pop 21, call double_it, push 42
        FIN,
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.register_extern(0, double_it);
    vm.load(&prog).unwrap();

    assert_eq!(vm.global(0).int_value(), 42);
}

#[test]
fn test_extern_not_registered() {
    let code = [
        INT, 1, 0, 0,
        EXTERN, 7, 0,
        FIN,
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);

    let result = vm.load(&prog);
    assert!(matches!(result, Err(VmError::NotRegistered(7))));
}
