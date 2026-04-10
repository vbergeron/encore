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
    // ENCORE pops fun, arg, cont from stack.
    let code = [
        PACK, 0,                    // cont = ctor(0) (dummy)
        PACK, 0,                    // arg = ctor(0)
        FUNCTION, 8, 0,             // function with code_ptr=8
        ENCORE,                     // pop clo, pop arg, pop cont, enter
        // function body at byte 8:
        ARG,                        // push arg register
        FIN,
    ];
    let arity_table = [0];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_load_capture() {
    let code = [
        PACK, 0,                    // cont = ctor(0) (dummy)
        PACK, 0,                    // arg = ctor(0)
        PACK, 1,                    // value to capture = ctor(1)
        CLOSURE, 11, 0, 1,          // closure with code_ptr=11, ncap=1
        ENCORE,                     // pop clo, pop arg, pop cont, enter
        // closure body at byte 11:
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
    // countdown = fix f (n, k). match n with Zero -> halt | Succ -> f (field 0 n) k
    //
    // ENCORE convention: pop clo (TOS), pop arg, pop cont.
    let code = [
        // main: build Succ(Succ(Zero))
        PACK, 0,                    // dummy cont
        PACK, 0,                    // Zero
        PACK, 1,                    // Succ(Zero)
        PACK, 1,                    // Succ(Succ(Zero))  — arg
        // build countdown function, enter
        FUNCTION, 12, 0,            // function code_ptr=12
        ENCORE,                     // pop clo, pop arg, pop cont, enter
        // countdown body at byte 12:
        ARG,                        // push arg
        MATCH, 0, 2,                // base=0, n=2
        20, 0,                      // off[0] = 20 (Zero branch)
        22, 0,                      // off[1] = 22 (Succ branch)
        // byte 20: Zero branch
        ARG,
        FIN,
        // byte 22: Succ branch
        CONT,                       // push cont (pass along)
        ARG,                        // push arg (the Succ ctor)
        FIELD, 0,                   // pop Succ(pred), push pred
        SELF,                       // push self
        ENCORE,                     // pop clo=self, pop arg=pred, pop cont, recurse
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
    // Three-phase program: ENCORE creates a garbage closure (with a dummy capture
    // to make it 3 words), garbage_func builds the real closure (also 3 words)
    // and RETURNs into it, and the real closure body allocates, triggering GC.
    // After GC compacts, CAPTURE must still read the correct capture.
    let code = [
        // main preamble:
        PACK, 0,                    // byte 0: dummy cont
        PACK, 0,                    // byte 2: arg = ctor(0)
        PACK, 1,                    // byte 4: dummy capture for garbage_func
        CLOSURE, 11, 0, 1,          // byte 6: garbage_func at byte 11, ncap=1. alloc 3. hp=3
        ENCORE,                     // byte 10: enter garbage_func

        // garbage_func body at byte 11:
        ARG,                        // byte 11: push result for RETURN
        PACK, 1,                    // byte 12: capture = ctor(1)
        CLOSURE, 19, 0, 1,          // byte 14: real closure at byte 19, ncap=1. alloc 3. hp=6
        RETURN,                     // byte 18: RETURN into real closure. garbage closure is now dead.

        // real_closure body at byte 19:
        ARG,                        // byte 19: push arg = ctor(0)
        PACK, 2,                    // byte 20: ctor(2, arity=1): pops ctor(0), alloc 2. GC!
        CAPTURE, 0,                 // byte 22: push capture 0 — should be ctor(1)
        FIN,                        // byte 24
    ];
    let arity_table = [0, 0, 1];
    let mut mem = [Value::from_u32(0); 8];
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
        FIN,                        // byte 9
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
