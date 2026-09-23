use encore_vm::error::VmError;
use encore_vm::opcode::*;
use encore_vm::program::{Program, MAGIC};
use encore_vm::value::{CodeAddress, GlobalAddress, Value};
use encore_vm::vm::Vm;

const A1: u8 = 2;
const X01: u8 = 10;

fn validate(code: &[u8], arities: &[u8], globals: &[u16]) -> Result<(), VmError> {
    let globals: Vec<CodeAddress> = globals.iter().map(|&g| CodeAddress::new(g)).collect();
    Program::new(code, arities, &globals).validate()
}

fn reason(result: Result<(), VmError>) -> &'static str {
    match result {
        Err(VmError::Invalid { reason, .. }) => reason,
        other => panic!("expected VmError::Invalid, got {other:?}"),
    }
}

// -- Accepted programs --

#[test]
fn test_empty_program_is_valid() {
    assert_eq!(validate(&[], &[], &[]), Ok(()));
}

#[test]
fn test_every_opcode_is_valid() {
    // Every jump targets the FIN at 0; the rest is only ever decoded.
    let code = [
        FIN, A1,
        MOV, X01, A1,
        CAPTURE, X01, 0,
        GLOBAL, X01, 0,
        CLOSURE, X01, 0, 0, 2, X01, A1,
        FUNCTION, X01, 0, 0,
        PACK, X01, 1, A1,
        FIELD, X01, A1, 0,
        UNPACK, X01, 1, A1,
        INT, X01, 1, 2, 3,
        INT_0, X01, INT_1, X01, INT_2, X01,
        INT_ADD, X01, A1, A1, INT_SUB, X01, A1, A1, INT_MUL, X01, A1, A1,
        INT_EQ, X01, A1, A1, INT_LT, X01, A1, A1, INT_BYTE, X01, A1,
        INT_LE, X01, A1, A1, INT_DIV, X01, A1, A1, INT_MOD, X01, A1, A1,
        INT_SUB_SAT, X01, A1, A1, INT_AND, X01, A1, A1, INT_OR, X01, A1, A1,
        INT_XOR, X01, A1, A1, INT_SHL, X01, A1, A1, INT_SHR, X01, A1, A1,
        GLOBAL_W, X01, 0, 0,
        EXTERN, X01, A1, 31, 0,
        BYTES, X01, 3, b'a', b'b', b'c',
        BYTES_LEN, X01, A1, BYTES_GET, X01, A1, A1,
        BYTES_CONCAT, X01, A1, A1, BYTES_SLICE, X01, A1, A1, A1, BYTES_EQ, X01, A1, A1,
        BRANCH, A1, 0, 0, 0, 0, 0,
        MATCH, A1, 0, 2, 0, 0, 0, 0,
        ENCORE, A1, CONT_REG,
    ];
    assert_eq!(validate(&code, &[0, 1], &[0]), Ok(()));
}
const CONT_REG: u8 = 1;

// -- Rejected programs --

#[test]
fn test_unknown_opcode() {
    assert_eq!(validate(&[0x3F, 0], &[], &[]), Err(VmError::InvalidOpcode { opcode: 0x3F, pc: 0 }));
    assert_eq!(validate(&[FIN, A1, NULL], &[], &[]), Err(VmError::InvalidOpcode { opcode: NULL, pc: 2 }));
}

#[test]
fn test_truncated_operands() {
    for code in [
        &[FIN][..],
        &[MOV, X01],
        &[INT, X01, 1, 2],
        &[CLOSURE, X01, 0, 0, 2, A1],
        &[BYTES, X01, 3, b'a'],
        &[MATCH, A1, 0, 2, 0, 0, 0],
        &[BRANCH, A1, 0, 0, 0, 0],
        &[PACK, X01, 0, A1],
    ] {
        assert_eq!(reason(validate(code, &[2], &[])), "instruction runs past end of code", "{code:?}");
    }
}

#[test]
fn test_falls_off_end() {
    assert_eq!(reason(validate(&[MOV, X01, A1], &[], &[])), "execution falls off the end of code");
    assert_eq!(reason(validate(&[FIN, A1, INT_0, X01], &[], &[])), "execution falls off the end of code");
}

#[test]
fn test_jump_target_outside_code() {
    let code = [FUNCTION, X01, 6, 0, FIN, X01];
    assert_eq!(validate(&code, &[], &[]), Err(VmError::Invalid { pc: 0, reason: "jump target outside code" }));
    let code = [FIN, A1, BRANCH, A1, 0, 0, 0, 9, 0];
    assert_eq!(validate(&code, &[], &[]), Err(VmError::Invalid { pc: 2, reason: "jump target outside code" }));
}

#[test]
fn test_jump_target_mid_instruction() {
    // FUNCTION points at the operand byte of FIN.
    let code = [FUNCTION, X01, 5, 0, FIN, X01];
    assert_eq!(validate(&code, &[], &[]),
        Err(VmError::Invalid { pc: 0, reason: "jump target not on an instruction boundary" }));
    // MATCH table entry into the middle of its own table.
    let code = [MATCH, A1, 0, 2, 8, 0, 5, 0, FIN, A1];
    assert_eq!(reason(validate(&code, &[], &[])), "jump target not on an instruction boundary");
    // CLOSURE code pointer.
    let code = [CLOSURE, X01, 1, 0, 0, FIN, X01];
    assert_eq!(reason(validate(&code, &[], &[])), "jump target not on an instruction boundary");
}

#[test]
fn test_many_jump_targets() {
    // More targets than one boundary walk checks, all valid, then one bad one last.
    let mut code = Vec::new();
    for _ in 0..300 {
        code.extend_from_slice(&[FUNCTION, X01, 0, 0]);
    }
    code.extend_from_slice(&[FIN, X01]);
    assert_eq!(validate(&code, &[], &[0]), Ok(()));
    let n = code.len();
    code[n - 2 - 4 + 2] = 1; // last FUNCTION -> address 1
    assert_eq!(validate(&code, &[], &[0]),
        Err(VmError::Invalid { pc: (n - 6) as u16, reason: "jump target not on an instruction boundary" }));
}

#[test]
fn test_global_entry_points() {
    let code = [FIN, A1, FIN, A1];
    assert_eq!(validate(&code, &[], &[0, 2]), Ok(()));
    assert_eq!(reason(validate(&code, &[], &[1])), "jump target not on an instruction boundary");
    assert_eq!(reason(validate(&code, &[], &[4])), "global entry point outside code");
}

#[test]
fn test_tag_outside_arity_table() {
    assert_eq!(reason(validate(&[PACK, X01, 1, FIN, X01], &[0], &[])), "constructor tag outside the arity table");
    assert_eq!(reason(validate(&[UNPACK, X01, 0, A1, FIN, X01], &[], &[])), "constructor tag outside the arity table");
}

#[test]
fn test_unpack_past_register_file() {
    assert_eq!(validate(&[UNPACK, 0xFE, 0, A1, FIN, X01], &[2], &[]), Ok(()));
    assert_eq!(reason(validate(&[UNPACK, 0xFF, 0, A1, FIN, X01], &[2], &[])), "UNPACK writes past the register file");
}

#[test]
fn test_global_index_out_of_range() {
    let code = [GLOBAL, X01, 1, FIN, X01];
    assert_eq!(reason(validate(&code, &[], &[0])), "global index out of range");
    assert_eq!(validate(&code, &[], &[0, 0]), Ok(()));
}

#[test]
fn test_extern_slot_out_of_range() {
    assert_eq!(reason(validate(&[EXTERN, X01, A1, 32, 0, FIN, X01], &[], &[])), "extern slot out of range");
}

#[test]
fn test_wide_global_index() {
    let code = [GLOBAL_W, X01, 0, 1, FIN, X01];
    let globals = vec![0u16; 256];
    assert_eq!(reason(validate(&code, &[], &globals)), "global index out of range");
    let globals = vec![0u16; 257];
    assert_eq!(validate(&code, &[], &globals), Ok(()));
    assert_eq!(reason(validate(&[GLOBAL_W, X01, 0], &[], &[0])), "instruction runs past end of code");
}

// -- Loading --

#[test]
fn test_load_rejects_invalid_code_before_running() {
    // The global would run fine, but trailing garbage makes the program invalid.
    let code = [FIN, A1, 0x3F];
    let globals = [CodeAddress::new(0)];
    let prog = Program::new(&code, &[], &globals);
    let mut mem = [Value::ZERO; 64];
    let mut vm = Vm::init(&mut mem);
    assert!(matches!(vm.load(&prog), Err(VmError::InvalidOpcode { opcode: 0x3F, pc: 2 })));
}

#[test]
fn test_call_null_continuation_is_an_error() {
    // Global 0: X01 = function(@6); FIN X01. Body at 6: ENCORE NULL, NULL.
    let code = [FUNCTION, X01, 6, 0, FIN, X01, ENCORE, NULL, NULL];
    let globals = [CodeAddress::new(0)];
    let prog = Program::new(&code, &[], &globals);
    let mut mem = [Value::ZERO; 64];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    let err = vm.call_global_raw(GlobalAddress::new(0), &[]).unwrap_err();
    assert_eq!(err, VmError::Invalid { pc: 6, reason: "call target outside code" });
}

#[test]
fn test_call_missing_global_is_an_error() {
    let code = [FIN, A1];
    let globals = [CodeAddress::new(0)];
    let prog = Program::new(&code, &[], &globals);
    let mut mem = [Value::ZERO; 64];
    let mut vm = Vm::init(&mut mem);
    vm.load(&prog).unwrap();
    assert_eq!(reason(vm.call_global_raw(GlobalAddress::new(1), &[]).map(|_| ())), "global index out of range");
}

// -- Fuzzing --

/// xorshift64*: deterministic, dependency-free randomness.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn header(n_arities: usize, n_globals: usize, code_len: usize) -> Vec<u8> {
    let mut bytes = MAGIC.to_vec();
    for n in [n_arities, n_globals, code_len] {
        bytes.extend_from_slice(&(n as u16).to_le_bytes());
    }
    bytes
}

/// Fuzz iterations; Miri runs are far slower, so they take a sample.
fn iterations(n: usize) -> usize {
    if cfg!(miri) { n / 100 } else { n }
}

/// Parsing and validating must return, never panic, whatever the input.
fn check(bytes: &[u8]) -> bool {
    match Program::parse(bytes) {
        Ok(prog) => prog.validate().is_ok(),
        Err(_) => false,
    }
}

#[test]
fn test_fuzz_random_bytes() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..iterations(20_000) {
        let len = rng.below(64);
        let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        check(&bytes);
    }
}

#[test]
fn test_fuzz_random_code() {
    // A well-formed header over random code, biased toward real opcodes so
    // decoding gets past the first byte.
    let ops = [
        FIN, MOV, CAPTURE, GLOBAL, CLOSURE, PACK, FIELD, MATCH, ENCORE, BRANCH,
        FUNCTION, UNPACK, INT, INT_ADD, INT_BYTE, INT_0, EXTERN, BYTES, BYTES_SLICE,
    ];
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let mut accepted = 0;
    for _ in 0..iterations(50_000) {
        let n_arities = rng.below(4);
        let n_globals = rng.below(3);
        let code_len = rng.below(48);
        let mut bytes = header(n_arities, n_globals, code_len);
        bytes.extend((0..n_arities).map(|_| rng.below(4) as u8));
        for _ in 0..n_globals {
            bytes.extend_from_slice(&(rng.below(code_len + 1) as u16).to_le_bytes());
        }
        for _ in 0..code_len {
            let byte = match rng.below(3) {
                0 => ops[rng.below(ops.len())],
                1 => rng.below(code_len + 1) as u8,
                _ => rng.next() as u8,
            };
            bytes.push(byte);
        }
        if check(&bytes) { accepted += 1; }
    }
    assert!(accepted > 0, "fuzzer never produced a valid program");
}

#[test]
fn test_fuzz_mutated_program() {
    let code = [
        FIN, A1,                        // 0
        FUNCTION, X01, 0, 0,            // 2
        PACK, X01, 1, A1,               // 6
        BRANCH, X01, 0, 17, 0, 0, 0,    // 10
        FIN, X01,                       // 17
        MATCH, A1, 0, 2, 17, 0, 0, 0,   // 19
    ];
    let mut base = header(2, 1, code.len());
    base.extend_from_slice(&[0, 1]);
    base.extend_from_slice(&2u16.to_le_bytes());
    base.extend_from_slice(&code);
    assert!(check(&base));

    let mut rng = Rng(0x0123_4567_89AB_CDEF);
    for _ in 0..iterations(50_000) {
        let mut bytes = base.clone();
        for _ in 0..1 + rng.below(3) {
            let i = rng.below(bytes.len());
            bytes[i] = rng.next() as u8;
        }
        if rng.below(4) == 0 {
            bytes.truncate(rng.below(bytes.len() + 1));
        }
        check(&bytes);
    }
}
