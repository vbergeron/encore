use encore_vm::error::VmError;
use encore_vm::program::{Program, MAGIC};
use encore_vm::value::Value;

fn build(n_arities: u16, n_globals: u16, arities: &[u8], globals: &[u32], code: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&MAGIC);
    buf.extend_from_slice(&n_arities.to_le_bytes());
    buf.extend_from_slice(&n_globals.to_le_bytes());
    buf.extend_from_slice(&(code.len() as u16).to_le_bytes());
    buf.extend_from_slice(arities);
    for &g in globals {
        buf.extend_from_slice(&g.to_le_bytes());
    }
    buf.extend_from_slice(code);
    buf
}

#[test]
fn test_empty_program() {
    let bytes = build(0, 0, &[], &[], &[]);
    let prog = Program::parse(&bytes).unwrap();
    assert_eq!(prog.arity_table.len(), 0);
    assert_eq!(prog.code.len(), 0);
    assert_eq!(prog.n_globals(), 0);
}

#[test]
fn test_arity_table() {
    let bytes = build(3, 0, &[0, 2, 1], &[], &[]);
    let prog = Program::parse(&bytes).unwrap();
    assert_eq!(prog.arity_table, &[0, 2, 1]);
}

#[test]
fn test_globals_roundtrip() {
    let g0 = Value::from_u32(0xDEAD_BEEF);
    let g1 = Value::from_u32(42);
    let bytes = build(0, 2, &[], &[g0.to_u32(), g1.to_u32()], &[]);
    let prog = Program::parse(&bytes).unwrap();
    assert_eq!(prog.n_globals(), 2);
    assert_eq!(prog.global(0).to_u32(), 0xDEAD_BEEF);
    assert_eq!(prog.global(1).to_u32(), 42);
}

#[test]
fn test_load_globals() {
    let bytes = build(0, 2, &[], &[100, 200], &[]);
    let prog = Program::parse(&bytes).unwrap();
    let mut buf = [Value::from_u32(0); 2];
    prog.load_globals(&mut buf);
    assert_eq!(buf[0].to_u32(), 100);
    assert_eq!(buf[1].to_u32(), 200);
}

#[test]
fn test_code_slice() {
    let code = &[0x01, 0x02, 0x03, 0xFF];
    let bytes = build(0, 0, &[], &[], code);
    let prog = Program::parse(&bytes).unwrap();
    assert_eq!(prog.code, code);
}

#[test]
fn test_full_program() {
    let arities = &[0, 2];
    let globals = &[0xCAFE_BABE];
    let code = &[0xAA, 0xBB];
    let bytes = build(2, 1, arities, globals, code);
    let prog = Program::parse(&bytes).unwrap();
    assert_eq!(prog.arity_table, arities);
    assert_eq!(prog.n_globals(), 1);
    assert_eq!(prog.global(0).to_u32(), 0xCAFE_BABE);
    assert_eq!(prog.code, code);
}

#[test]
fn test_bad_magic() {
    let mut bytes = build(0, 0, &[], &[], &[]);
    bytes[0] = b'X';
    assert_eq!(Program::parse(&bytes).unwrap_err(), VmError::BadMagic);
}

#[test]
fn test_truncated_header() {
    assert_eq!(Program::parse(&[]).unwrap_err(), VmError::Truncated);
    assert_eq!(Program::parse(&MAGIC).unwrap_err(), VmError::Truncated);
    assert_eq!(Program::parse(&[0; 9]).unwrap_err(), VmError::Truncated);
}

#[test]
fn test_truncated_payload() {
    let mut bytes = build(3, 0, &[1, 2, 3], &[], &[]);
    bytes.pop();
    assert_eq!(Program::parse(&bytes).unwrap_err(), VmError::Truncated);
}

#[test]
fn test_extra_trailing_bytes_ignored() {
    let mut bytes = build(0, 0, &[], &[], &[0xFF]);
    bytes.extend_from_slice(&[0x00; 100]);
    let prog = Program::parse(&bytes).unwrap();
    assert_eq!(prog.code, &[0xFF]);
}
