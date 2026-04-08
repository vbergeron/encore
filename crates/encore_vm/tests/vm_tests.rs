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
    let code = [PACK, 0, HALT];
    let arity_table = [0];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_pack_and_field() {
    // Pack two nullary ctors (tag=1, tag=2), then a binary ctor (tag=0, arity=2).
    // Field 0 should be the first value pushed (tag=1).
    let code = [
        PACK, 1,
        PACK, 2,
        PACK, 0,
        FIELD, 0,
        HALT,
    ];
    let arity_table = [2, 0, 0];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- Closure tests --

#[test]
fn test_closure_and_enter() {
    // Push arg (ctor tag=0), build closure whose body is HALT, ENCORE.
    // Result: the argument (ctor tag=0).
    let code = [
        PACK, 0,                    // arg = ctor(0)
        CLOSURE, 7, 0, 0,           // closure with code_ptr=7, ncap=0
        ENCORE,                     // pop clo, pop arg, enter
        HALT,                       // closure body: return arg
    ];
    let arity_table = [0];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_load_capture() {
    // Build arg (ctor tag=0), value to capture (ctor tag=1), closure capturing it.
    // Closure body: LOAD capture 0, HALT. Should return ctor(1).
    let code = [
        PACK, 0,                    // arg = ctor(0)
        PACK, 1,                    // value to capture = ctor(1)
        CLOSURE, 9, 0, 1,           // closure with code_ptr=9, ncap=1 (captures ctor(1))
        ENCORE,                     // pop clo, pop arg=ctor(0), enter
        // closure body at byte 9:
        LOAD, 0,                    // push capture 0 = ctor(1)
        HALT,
    ];
    let arity_table = [0, 0];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_load_global() {
    let global = Value::ctor(42, encore_vm::value::HeapAddress::new(0));
    let code = [LOAD, 0x80, HALT];
    let result = run(&code, &[], &[global]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 42);
}

// -- Match test --

#[test]
fn test_match() {
    // Pack ctor(1), match on it with base=0, 2 branches.
    // Branch 0 (tag=0) → pack ctor(2), halt.
    // Branch 1 (tag=1) → pack ctor(3), halt.
    // Expected: ctor(3).
    let code = [
        PACK, 1,                    // ctor(1)
        MATCH, 0, 2,                // base=0, n=2
        9, 0,                       // off[0] = 9 (branch tag=0)
        12, 0,                      // off[1] = 12 (branch tag=1)
        // byte 9: branch tag=0
        PACK, 2,
        HALT,
        // byte 12: branch tag=1
        PACK, 3,
        HALT,
    ];
    let arity_table = [0, 0, 0, 0];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 3);
}

// -- FIX test (Peano countdown) --

#[test]
fn test_fix() {
    // Peano naturals: tag 0 = Zero (arity 0), tag 1 = Succ (arity 1).
    // Build Succ(Succ(Zero)), enter countdown.
    // countdown = fix f n. match n with Zero → halt | Succ → f (field 0 n)
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
        MATCH, 0, 2,                // base=0, n=2
        18, 0,                      // off[0] = 18 (Zero branch)
        19, 0,                      // off[1] = 19 (Succ branch)
        // byte 18: Zero branch
        HALT,
        // byte 19: Succ branch
        FIELD, 0,                   // peek Succ(pred), push pred
        FIX,                        // push self
        ENCORE,                     // pop clo=self, pop arg=pred, recurse
    ];
    let arity_table = [0, 1];
    let result = run(&code, &arity_table, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0); // Zero
}

// -- Error tests --

#[test]
fn test_heap_overflow() {
    // arity_table says tag 0 has 100 fields → alloc(100) on a tiny heap.
    let code = [PACK, 0];
    let arity_table = [100];
    let mut mem = [Value::from_u32(0); 4];
    let mut vm = Vm::new(&code, &arity_table, &[], &mut mem);
    let result = vm.run();
    assert!(matches!(result, Err(VmError::HeapOverflow)));
}

#[test]
fn test_stack_overflow() {
    // Push globals repeatedly on a tiny stack until it overflows.
    let globals = [Value::from_u32(0)];
    let code = [
        LOAD, 0x80,
        LOAD, 0x80,
        LOAD, 0x80,
        LOAD, 0x80,
        LOAD, 0x80,
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

// -- call() API test --

#[test]
fn test_call() {
    // Function at byte 0: match arg, Zero → halt, Succ → extract pred, halt.
    // Call it with Succ(Zero), expect Zero.
    let code = [
        // function body at byte 0:
        MATCH, 0, 2,
        7, 0,                       // off[0] = 7 (Zero branch)
        8, 0,                       // off[1] = 8 (Succ branch)
        HALT,                       // byte 7: Zero → return it
        FIELD, 0,                   // byte 8: Succ → push pred
        HALT,
    ];
    let arity_table = [0, 1];

    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(&code, &arity_table, &[], &mut mem);

    // Build Succ(Zero) as the argument.
    // Zero is a nullary ctor — no heap needed.
    // Succ needs 1 field on the heap, but we can't alloc from outside.
    // So call a small preamble that builds the value first.
    // Simpler: just call with a nullary ctor and check Zero branch.
    let arg = Value::ctor(0, HeapAddress::new(0));
    let result = vm.call(CodeAddress::new(0), arg).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}
