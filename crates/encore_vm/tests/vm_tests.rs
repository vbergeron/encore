use encore_vm::error::VmError;
use encore_vm::opcode::*;
use encore_vm::value::{CodeAddress, HeapAddress, Value};
use encore_vm::vm::Vm;

fn run(code: &[u8], arity_table: &[u8], globals: &[Value]) -> Result<Value, VmError> {
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(code, arity_table, globals, &mut mem);
    vm.run()
}

// -- Basic tests --

#[test]
fn test_pack_nullary() {
    let code = [PACK, 0, FIN];
    let arity_table = [0];
    let result = run(&code, &arity_table, &[]).unwrap();
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
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- Closure tests --

#[test]
fn test_closure_and_enter() {
    // Build arg (ctor tag=0), build closure whose body is ARG FIN, ENCORE.
    let code = [
        PACK, 0,                    // arg = ctor(0)
        CLOSURE, 7, 0, 0,           // closure with code_ptr=7, ncap=0
        ENCORE,                     // pop clo, pop arg, enter
        ARG,                        // closure body: push arg register
        FIN,
    ];
    let arity_table = [0];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_load_capture() {
    // Build arg (ctor tag=0), value to capture (ctor tag=1), closure capturing it.
    // Closure body: CAPTURE 0, FIN. Should return ctor(1).
    let code = [
        PACK, 0,                    // arg = ctor(0)
        PACK, 1,                    // value to capture = ctor(1)
        CLOSURE, 9, 0, 1,           // closure with code_ptr=9, ncap=1 (captures ctor(1))
        ENCORE,                     // pop clo, pop arg=ctor(0), enter
        // closure body at byte 9:
        CAPTURE, 0,                 // push capture 0 = ctor(1)
        FIN,
    ];
    let arity_table = [0, 0];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_load_global() {
    let global = Value::ctor(42, HeapAddress::new(0));
    let code = [GLOBAL, 0, FIN];
    let result = run(&code, &[], &[global]).unwrap();
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
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 3);
}

// -- SELF test (Peano countdown) --

#[test]
fn test_self_recursive() {
    // Peano naturals: tag 0 = Zero (arity 0), tag 1 = Succ (arity 1).
    // Build Succ(Succ(Zero)), enter countdown.
    // countdown = fix f n. match n with Zero -> halt | Succ -> f (field 0 n)
    //
    // ENCORE convention: pop clo (TOS), pop arg (below).
    // So: push arg, push clo, ENCORE.
    let code = [
        // main: build Succ(Succ(Zero))
        PACK, 0,                    // Zero
        PACK, 1,                    // Succ(Zero)
        PACK, 1,                    // Succ(Succ(Zero))  — arg
        // build countdown closure, enter
        CLOSURE, 11, 0, 0,          // closure code_ptr=11, ncap=0
        ENCORE,                     // pop clo, pop arg, enter
        // countdown body at byte 11:
        ARG,                        // push arg
        MATCH, 0, 2,                // base=0, n=2
        19, 0,                      // off[0] = 19 (Zero branch)
        21, 0,                      // off[1] = 21 (Succ branch)
        // byte 19: Zero branch
        ARG,
        FIN,
        // byte 21: Succ branch
        ARG,                        // push arg (the Succ ctor)
        FIELD, 0,                   // peek Succ(pred), push pred
        SELF,                       // push self
        ENCORE,                     // pop clo=self, pop arg=pred, recurse
    ];
    let arity_table = [0, 1];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0); // Zero
}

// -- Integer tests --

#[test]
fn test_int_const() {
    let code = [INT, 42, 0, 0, FIN];
    let result = run(&code, &[], &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value(), 42);
}

#[test]
fn test_int_const_negative() {
    // -1 in 24-bit two's complement: 0xFF_FFFF
    let code = [INT, 0xFF, 0xFF, 0xFF, FIN];
    let result = run(&code, &[], &[]).unwrap();
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
    let result = run(&code, &[], &[]).unwrap();
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
    let result = run(&code, &[], &[]).unwrap();
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
    let result = run(&code, &[], &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value(), 42);
}

#[test]
fn test_int_eq_true() {
    let code = [
        INT, 5, 0, 0,
        INT, 5, 0, 0,
        INT_EQ,
        FIN,
    ];
    let result = run(&code, &[], &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_int_eq_false() {
    let code = [
        INT, 5, 0, 0,
        INT, 6, 0, 0,
        INT_EQ,
        FIN,
    ];
    let result = run(&code, &[], &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_int_lt_true() {
    let code = [
        INT, 3, 0, 0,
        INT, 5, 0, 0,
        INT_LT,
        FIN,
    ];
    let result = run(&code, &[], &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_int_lt_false() {
    let code = [
        INT, 5, 0, 0,
        INT, 3, 0, 0,
        INT_LT,
        FIN,
    ];
    let result = run(&code, &[], &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Error tests --

#[test]
fn test_heap_overflow() {
    let code = [PACK, 0];
    let arity_table = [100];
    let mut mem = [Value::from_u32(0); 4];
    let mut vm = Vm::new(&code, &arity_table, &[], &mut mem);
    let result = vm.run();
    assert!(matches!(result, Err(VmError::HeapOverflow)));
}

#[test]
fn test_stack_overflow() {
    let globals = [Value::from_u32(0)];
    let code = [
        GLOBAL, 0,
        GLOBAL, 0,
        GLOBAL, 0,
        GLOBAL, 0,
        GLOBAL, 0,
    ];
    let mut mem = [Value::from_u32(0); 4];
    let mut vm = Vm::new(&code, &[], &globals, &mut mem);
    let result = vm.run();
    assert!(matches!(result, Err(VmError::StackOverflow)));
}

#[test]
fn test_invalid_opcode() {
    let code = [0xFF];
    let result = run(&code, &[], &[]);
    assert!(matches!(result, Err(VmError::InvalidOpcode(0xFF))));
}

// -- GC tests --

#[test]
fn test_gc_reclaims_dead_closures() {
    let code = [ARG, FIN];
    let mut mem = [Value::from_u32(0); 10];
    let mut vm = Vm::new(&code, &[], &[], &mut mem);
    let arg = Value::ctor(0, HeapAddress::NULL);
    for _ in 0..10 {
        let result = vm.call(CodeAddress::new(0), arg).unwrap();
        assert!(result.is_ctor());
        assert_eq!(result.ctor_tag(), 0);
    }
}

#[test]
fn test_gc_preserves_live_data() {
    // Two-phase program: first ENCORE creates a garbage closure,
    // second ENCORE enters the real closure (with a capture).
    // The real closure's body allocates, triggering GC.
    // After GC compacts, CAPTURE must still read the correct capture.
    let code = [
        // main preamble:
        PACK, 0,                    // byte 0: arg = ctor(0), nullary, no heap
        CLOSURE, 7, 0, 0,           // byte 2: garbage_func at byte 7, ncap=0. alloc 2. hp=2
        ENCORE,                     // byte 6: enter garbage_func

        // garbage_func body at byte 7:
        ARG,                        // byte 7: push arg for next ENCORE
        PACK, 1,                    // byte 8: capture = ctor(1), nullary, no heap
        CLOSURE, 15, 0, 1,          // byte 10: real closure at byte 15, ncap=1. alloc 3. hp=5
        ENCORE,                     // byte 14: enter real closure. garbage closure is now dead.

        // real_closure body at byte 15:
        ARG,                        // byte 15: push arg = ctor(0)
        PACK, 2,                    // byte 16: ctor(2, arity=1): pops ctor(0), alloc 2. GC!
        CAPTURE, 0,                 // byte 18: push capture 0 — should be ctor(1)
        FIN,                       // byte 20
    ];
    let arity_table = [0, 0, 1];
    let mut mem = [Value::from_u32(0); 7];
    let mut vm = Vm::new(&code, &arity_table, &[], &mut mem);
    let result = vm.run().unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- call() API test --

#[test]
fn test_call() {
    // Function at byte 0: match arg, Zero -> halt, Succ -> extract pred, halt.
    let code = [
        // function body at byte 0:
        ARG,                        // push arg
        MATCH, 0, 2,
        8, 0,                       // off[0] = 8 (Zero branch)
        10, 0,                      // off[1] = 10 (Succ branch)
        ARG,                        // byte 8: Zero -> push arg
        FIN,                       // byte 9
        ARG,                        // byte 10: Succ -> push arg
        FIELD, 0,                   // push pred
        FIN,
    ];
    let arity_table = [0, 1];

    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(&code, &arity_table, &[], &mut mem);

    let arg = Value::ctor(0, HeapAddress::new(0));
    let result = vm.call(CodeAddress::new(0), arg).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}
