use encore_vm::error::{ExternError, VmError};
use encore_vm::opcode::*;
use encore_vm::program::Program;
use encore_vm::value::{CodeAddress, GlobalAddress, HeapAddress, Value};
use encore_vm::vm::Vm;

fn run(code: &[u8], arity_table: &[u8]) -> Result<Value, VmError> {
    let prog = Program::new(code, arity_table, &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog)?;
    Ok(vm.global(0))
}

const X01: u8 = 10;
const X02: u8 = 11;
const X03: u8 = 12;
const X04: u8 = 13;
const X05: u8 = 14;

// -- Basic tests --

#[test]
fn test_pack_nullary() {
    // PACK X01, tag=0; FIN X01
    let code = [PACK, X01, 0, FIN, X01];
    let result = run(&code, &[0]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_pack_and_field() {
    let code = [
        PACK, X01, 1,                // X01 = ctor(1) nullary
        PACK, X02, 2,                // X02 = ctor(2) nullary
        PACK, X03, 0, X01, X02,      // X03 = ctor(0, X01, X02)  arity=2
        FIELD, X04, X03, 0,          // X04 = field 0 of X03 = ctor(1)
        FIN, X04,
    ];
    let result = run(&code, &[2, 0, 0]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

// -- Closure tests --

#[test]
fn test_closure_and_enter() {
    let code = [
        PACK, X01, 0,               // 0-2:  X01 = dummy cont
        PACK, X02, 0,               // 3-5:  X02 = arg
        FUNCTION, X03, 16, 0,       // 6-9:  X03 = function(@16)
        MOV, 2, X02,                // 10-12: MOV A1, X02
        ENCORE, X03, X01,           // 13-15: ENCORE X03, X01
        // function body at byte 16:
        FIN, 2,                     // FIN A1
    ];
    let result = run(&code, &[0]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_load_capture() {
    let code = [
        PACK, X01, 0,                   // 0-2:  X01 = dummy cont
        PACK, X02, 0,                   // 3-5:  X02 = arg = ctor(0)
        PACK, X03, 1,                   // 6-8:  X03 = ctor(1) to capture
        CLOSURE, X04, 21, 0, 1, X03,   // 9-14: X04 = closure(@21, ncap=1, caps=[X03])
        MOV, 2, X02,                    // 15-17: MOV A1, X02
        ENCORE, X04, X01,               // 18-20: ENCORE X04, X01
        // closure body at byte 21:
        CAPTURE, X01, 0,                // X01 = capture 0 = ctor(1)
        FIN, X01,
    ];
    let result = run(&code, &[0, 0]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_load_global() {
    let code = [
        // global 0 thunk: produce ctor(42)
        PACK, X01, 42, FIN, X01,
        // global 1 thunk: read global 0
        GLOBAL, X01, 0, FIN, X01,
    ];
    let arity_table = [0; 43];
    let prog = Program::new(&code, &arity_table, &[CodeAddress::new(0), CodeAddress::new(5)]);
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
        PACK, X01, 1,                    // 0-2: X01 = ctor(1)
        MATCH, X01, 0, 2,                // 3-6: match
        11, 0,                           // 7-8: off[0] = 11
        16, 0,                           // 9-10: off[1] = 16
        // byte 11: branch tag=0
        PACK, X01, 2,                    // 11-13
        FIN, X01,                        // 14-15
        // byte 16: branch tag=1
        PACK, X01, 3,                    // 16-18
        FIN, X01,                        // 19-20
    ];
    let result = run(&code, &[0, 0, 0, 0]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 3);
}

// -- SELF test (Peano countdown) --

#[test]
fn test_self_recursive() {
    let code = [
        PACK, X01, 0,                    // 0-2:   dummy cont
        PACK, X02, 0,                    // 3-5:   Zero
        PACK, X03, 1, X02,              // 6-9:   Succ(Zero)
        PACK, X04, 1, X03,              // 10-13:  Succ(Succ(Zero))
        FUNCTION, X05, 24, 0,           // 14-17:  countdown function @24
        MOV, 2, X04,                     // 18-20:  MOV A1, X04
        ENCORE, X05, X01,               // 21-23:  ENCORE X05, X01
        // countdown body at byte 24:
        MATCH, 2, 0, 2,                 // 24-27:  match A1, base=0, n=2
        32, 0,                           // 28-29:  off[0] = 32 (Zero)
        34, 0,                           // 30-31:  off[1] = 34 (Succ)
        // byte 32: Zero branch
        FIN, 2,                          // 32-33:  FIN A1
        // byte 34: Succ branch
        FIELD, X01, 2, 0,               // 34-37:  X01 = field 0 of A1 = pred
        MOV, 2, X01,                     // 38-40:  MOV A1, X01
        ENCORE, 0, 1,                    // 41-43:  ENCORE SELF, CONT
    ];
    let result = run(&code, &[0, 1]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0); // Zero
}

// -- Integer tests --

#[test]
fn test_int_const() {
    let code = [INT, X01, 42, 0, 0, FIN, X01];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_int_const_negative() {
    let code = [INT, X01, 0xFF, 0xFF, 0xFF, FIN, X01];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), -1);
}

#[test]
fn test_int_add() {
    let code = [
        INT, X01, 3, 0, 0,
        INT, X02, 4, 0, 0,
        INT_ADD, X03, X01, X02,
        FIN, X03,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_int_sub() {
    let code = [
        INT, X01, 10, 0, 0,
        INT, X02, 3, 0, 0,
        INT_SUB, X03, X01, X02,
        FIN, X03,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 7);
}

#[test]
fn test_int_mul() {
    let code = [
        INT, X01, 6, 0, 0,
        INT, X02, 7, 0, 0,
        INT_MUL, X03, X01, X02,
        FIN, X03,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 42);
}

#[test]
fn test_int_eq_true() {
    let code = [INT, X01, 5, 0, 0, INT, X02, 5, 0, 0, INT_EQ, X03, X01, X02, FIN, X03];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_int_eq_false() {
    let code = [INT, X01, 5, 0, 0, INT, X02, 6, 0, 0, INT_EQ, X03, X01, X02, FIN, X03];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_int_lt_true() {
    let code = [INT, X01, 3, 0, 0, INT, X02, 5, 0, 0, INT_LT, X03, X01, X02, FIN, X03];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_int_lt_false() {
    let code = [INT, X01, 5, 0, 0, INT, X02, 3, 0, 0, INT_LT, X03, X01, X02, FIN, X03];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Error tests --

#[test]
fn test_heap_overflow() {
    // PACK with arity 3 needs 4 words, mem only has 3
    let code = [PACK, X01, 0, X01, X01, X01, FIN, X01];
    let arity_table = [3];
    let prog = Program::new(&code, &arity_table, &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 3];
    let mut vm = Vm::init(&mut mem);
    let result = vm.load(&prog);
    assert!(matches!(result, Err(VmError::HeapOverflow)));
}

#[test]
fn test_invalid_opcode() {
    let code = [0xF0];
    let result = run(&code, &[]);
    assert!(matches!(result, Err(VmError::InvalidOpcode { opcode: 0xF0, .. })));
}

// -- GC tests --

#[test]
fn test_gc_reclaims_dead_closures() {
    let code = [
        // global 0 thunk: produce function(@6)
        FUNCTION, X01, 6, 0, FIN, X01,
        // function body at offset 6:
        FIN, 2,                     // FIN A1
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 10];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let arg = Value::ctor(0, HeapAddress::NULL);
    for _ in 0..10 {
        let result = vm.call_global_raw(GlobalAddress::new(0), &[arg]).unwrap();
        assert!(result.is_ctor());
        assert_eq!(result.ctor_tag(), 0);
    }
}

#[test]
fn test_gc_preserves_live_data() {
    let code = [
        // thunk: create a closure capturing ctor(1), enter it
        PACK, X01, 1,                       // 0-2:   X01 = ctor(1) to capture
        CLOSURE, X02, 20, 0, 1, X01,       // 3-8:   X02 = closure(@20, caps=[X01])
        INT_0, X01,                          // 9-10:  clear stale X01
        PACK, X03, 0,                       // 11-13: X03 = dummy arg ctor(0)
        MOV, 2, X03,                         // 14-16: MOV A1, X03
        ENCORE, X02, NULL,                   // 17-19: ENCORE X02, NULL

        // closure body at byte 20:
        PACK, X01, 2, 2,                    // 20-23: X01 = ctor(2, [A1]), allocs 2
        INT_0, X01,                          // 24-25: kill X01, make ctor dead
        PACK, X01, 2, 2,                    // 26-29: X01 = ctor(2, [A1]), triggers GC
        CAPTURE, X02, 0,                    // 30-32: X02 = capture 0 = ctor(1)
        FIN, X02,                           // 33-34: return ctor(1)
    ];
    let arity_table = [0, 0, 1];
    let prog = Program::new(&code, &arity_table, &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 6];
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
        // global 0 thunk: produce function(@6)
        FUNCTION, X01, 6, 0,            // 0-3: X01 = function(@6)
        FIN, X01,                       // 4-5: return function
        // function body at offset 6:
        MATCH, 2, 0, 2,                 // 6-9: match A1, base=0, n=2
        14, 0,                          // 10-11: Zero branch at 14
        16, 0,                          // 12-13: Succ branch at 16
        // byte 14: Zero -> return A1
        FIN, 2,                         // 14-15
        // byte 16: Succ -> return pred
        FIELD, X01, 2, 0,              // 16-19: X01 = field 0 of A1
        FIN, X01,                       // 20-21
    ];
    let arity_table = [0, 1];
    let prog = Program::new(&code, &arity_table, &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();

    let arg = Value::ctor(0, HeapAddress::NULL);
    let result = vm.call_global_raw(GlobalAddress::new(0), &[arg]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

// -- Extern tests --

#[test]
fn test_extern_dispatch() {
    fn double_it(_vm: &mut Vm, v: Value) -> Result<Value, ExternError> {
        Ok(Value::int(v.int_value().unwrap() * 2))
    }

    let code = [
        INT, X01, 21, 0, 0,         // X01 = int(21)
        EXTERN, X02, X01, 0, 0,     // X02 = extern(0)(X01)
        FIN, X02,
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.register_extern(0, double_it);
    vm.load(&prog).unwrap();

    assert_eq!(vm.global(0).int_value().unwrap(), 42);
}

#[test]
fn test_extern_not_registered() {
    let code = [
        INT, X01, 1, 0, 0,
        EXTERN, X02, X01, 7, 0,
        FIN, X02,
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);

    assert!(matches!(vm.load(&prog), Err(VmError::Extern { .. })));
}

// -- Bytes tests --

#[test]
fn test_bytes_literal_empty() {
    let code = [BYTES, X01, 0, FIN, X01];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_bytes());
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    assert_eq!(vm.bytes_len(vm.global(0)), 0);
}

#[test]
fn test_bytes_literal() {
    let code = [BYTES, X01, 5, b'h', b'e', b'l', b'l', b'o', FIN, X01];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let val = vm.global(0);
    assert!(val.is_bytes());
    assert_eq!(vm.bytes_len(val), 5);
    assert_eq!(vm.bytes_read(val, 0), b'h');
    assert_eq!(vm.bytes_read(val, 1), b'e');
    assert_eq!(vm.bytes_read(val, 2), b'l');
    assert_eq!(vm.bytes_read(val, 3), b'l');
    assert_eq!(vm.bytes_read(val, 4), b'o');
}

#[test]
fn test_bytes_len() {
    let code = [
        BYTES, X01, 3, 0xAA, 0xBB, 0xCC,
        BYTES_LEN, X02, X01,
        FIN, X02,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 3);
}

#[test]
fn test_bytes_get() {
    let code = [
        BYTES, X01, 4, 10, 20, 30, 40,
        INT, X02, 2, 0, 0,
        BYTES_GET, X03, X01, X02,
        FIN, X03,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_int());
    assert_eq!(result.int_value().unwrap(), 30);
}

#[test]
fn test_bytes_concat() {
    let code = [
        BYTES, X01, 2, b'a', b'b',
        BYTES, X02, 3, b'c', b'd', b'e',
        BYTES_CONCAT, X03, X01, X02,
        FIN, X03,
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let val = vm.global(0);
    assert!(val.is_bytes());
    assert_eq!(vm.bytes_len(val), 5);
    assert_eq!(vm.bytes_read(val, 0), b'a');
    assert_eq!(vm.bytes_read(val, 1), b'b');
    assert_eq!(vm.bytes_read(val, 2), b'c');
    assert_eq!(vm.bytes_read(val, 3), b'd');
    assert_eq!(vm.bytes_read(val, 4), b'e');
}

#[test]
fn test_bytes_slice() {
    let code = [
        BYTES, X01, 5, b'h', b'e', b'l', b'l', b'o',
        INT, X02, 1, 0, 0,
        INT, X03, 3, 0, 0,
        BYTES_SLICE, X04, X01, X02, X03,
        FIN, X04,
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let val = vm.global(0);
    assert!(val.is_bytes());
    assert_eq!(vm.bytes_len(val), 3);
    assert_eq!(vm.bytes_read(val, 0), b'e');
    assert_eq!(vm.bytes_read(val, 1), b'l');
    assert_eq!(vm.bytes_read(val, 2), b'l');
}

#[test]
fn test_bytes_eq_true() {
    let code = [
        BYTES, X01, 3, 1, 2, 3,
        BYTES, X02, 3, 1, 2, 3,
        BYTES_EQ, X03, X01, X02,
        FIN, X03,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 1);
}

#[test]
fn test_bytes_eq_false_content() {
    let code = [
        BYTES, X01, 3, 1, 2, 3,
        BYTES, X02, 3, 1, 2, 4,
        BYTES_EQ, X03, X01, X02,
        FIN, X03,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_bytes_eq_false_length() {
    let code = [
        BYTES, X01, 2, 1, 2,
        BYTES, X02, 3, 1, 2, 3,
        BYTES_EQ, X03, X01, X02,
        FIN, X03,
    ];
    let result = run(&code, &[]).unwrap();
    assert!(result.is_ctor());
    assert_eq!(result.ctor_tag(), 0);
}

#[test]
fn test_bytes_gc_survives() {
    let code = [
        // global 0: a function that allocates bytes, triggers GC, returns captured bytes
        FUNCTION, X01, 6, 0, FIN, X01,
        // function body at offset 6:
        BYTES, X01, 4, b't', b'e', b's', b't',
        BYTES, X02, 4, b'j', b'u', b'n', b'k',
        INT_0, X02,
        BYTES, X02, 4, b'j', b'u', b'n', b'k',
        INT_0, X02,
        FIN, X01,
    ];
    let prog = Program::new(&code, &[], &[CodeAddress::new(0)]);
    let mut mem = [Value::from_u32(0); 12];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let arg = Value::int(0);
    let result = vm.call_global_raw(GlobalAddress::new(0), &[arg]).unwrap();
    assert!(result.is_bytes());
    assert_eq!(vm.bytes_len(result), 4);
    assert_eq!(vm.bytes_read(result, 0), b't');
    assert_eq!(vm.bytes_read(result, 1), b'e');
    assert_eq!(vm.bytes_read(result, 2), b's');
    assert_eq!(vm.bytes_read(result, 3), b't');
}
