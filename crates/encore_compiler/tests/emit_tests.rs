use encore_compiler::error::CompileError;
use encore_compiler::pass::asm_emit::Emitter;
use encore_compiler::ir::asm::*;
use encore_vm::opcode;

#[test]
fn test_fin_reg() {
    let expr = Expr::Fin(A1);
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();
    assert_eq!(code, [opcode::FIN, A1]);
}

#[test]
fn test_let_global_fin() {
    let expr = Expr::Let(X01, Val::Global(0), Box::new(Expr::Fin(X01)));
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();
    assert_eq!(code, [opcode::GLOBAL, X01, 0, opcode::FIN, X01]);
}

#[test]
fn test_let_wide_global_fin() {
    let expr = Expr::Let(X01, Val::Global(0x0123), Box::new(Expr::Fin(X01)));
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();
    assert_eq!(code, [opcode::GLOBAL_W, X01, 0x23, 0x01, opcode::FIN, X01]);
}

#[test]
fn test_bytes_literal_too_long_is_error() {
    let body = Expr::Let(X01, Val::Bytes(vec![0; 300]), Box::new(Expr::Fin(X01)));
    let module = Module { defines: vec![Define { global: 0, body }] };
    assert_eq!(
        Emitter::emit_module(&module, None),
        Err(CompileError::BytesLiteralTooLong { len: 300, max: 255 }),
    );
}

#[test]
fn test_too_many_captures_is_error() {
    let fun = Fun { captures: vec![X01; 200], body: Box::new(Expr::Fin(A1)) };
    let body = Expr::Letrec(X01, fun, Box::new(Expr::Fin(X01)));
    let module = Module { defines: vec![Define { global: 0, body }] };
    assert_eq!(
        Emitter::emit_module(&module, None),
        Err(CompileError::TooManyCaptures { count: 200, max: 125 }),
    );
}

#[test]
fn test_code_too_large_is_error() {
    // Each define is a 5-byte `INT`/`FIN`; 14k of them overflow 64 KB.
    let defines = (0..14_000)
        .map(|i| Define {
            global: i as u16,
            body: Expr::Let(X01, Val::Int(1000), Box::new(Expr::Fin(X01))),
        })
        .collect();
    let module = Module { defines };
    assert!(matches!(
        Emitter::emit_module(&module, None),
        Err(CompileError::CodeTooLarge { max: 65535, .. }),
    ));
}

#[test]
fn test_too_many_globals_is_error() {
    use encore_compiler::ir::ds;
    let defines = (0..70_000)
        .map(|i| ds::Define { name: format!("d{i}"), body: ds::Expr::Int(0) })
        .collect();
    let module = ds::Module { defines };
    assert_eq!(
        encore_compiler::pipeline::compile_module(module, None, None),
        Err(CompileError::TooManyGlobals { count: 70_000, max: 65535 }),
    );
}

#[test]
fn test_let_capture_fin() {
    let expr = Expr::Let(X01, Val::Capture(2), Box::new(Expr::Fin(X01)));
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();
    assert_eq!(code, [opcode::CAPTURE, X01, 2, opcode::FIN, X01]);
}

#[test]
fn test_mov() {
    let expr = Expr::Let(X01, Val::Reg(A1), Box::new(Expr::Fin(X01)));
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();
    assert_eq!(code, [opcode::MOV, X01, A1, opcode::FIN, X01]);
}

#[test]
fn test_match_two_branches() {
    let expr = Expr::Match(
        X01, 0,
        vec![
            Case { arity: 0, unpack_base: X01, body: Expr::Fin(A1) },
            Case { arity: 0, unpack_base: X01, body: Expr::Fin(CONT) },
        ],
    );
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();

    assert_eq!(code[0], opcode::BRANCH);
    assert_eq!(code[1], X01);  // scrutinee
    assert_eq!(code[2], 0);    // base tag
    let addr0 = u16::from_le_bytes([code[3], code[4]]);
    let addr1 = u16::from_le_bytes([code[5], code[6]]);
    assert_eq!(code[addr0 as usize], opcode::FIN);
    assert_eq!(code[addr0 as usize + 1], A1);
    assert_eq!(code[addr1 as usize], opcode::FIN);
    assert_eq!(code[addr1 as usize + 1], CONT);
}

#[test]
fn test_letrec_deferred_body() {
    let expr = Expr::Letrec(
        X01,
        Fun {
            captures: vec![CONT],
            body: Box::new(
                Expr::Let(X01, Val::Capture(0), Box::new(Expr::Fin(X01)))
            ),
        },
        Box::new(Expr::Fin(X01)),
    );

    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();

    assert_eq!(code[0], opcode::CLOSURE);
    assert_eq!(code[1], X01);  // dest
    let body_addr = u16::from_le_bytes([code[2], code[3]]);
    assert_eq!(code[4], 1);    // ncap
    assert_eq!(code[5], CONT); // capture register
    assert_eq!(code[6], opcode::FIN);
    assert_eq!(code[7], X01);
    // deferred body
    assert_eq!(body_addr, 8);
    assert_eq!(code[8], opcode::CAPTURE);
    assert_eq!(code[9], X01);
    assert_eq!(code[10], 0);   // capture index
    assert_eq!(code[11], opcode::FIN);
    assert_eq!(code[12], X01);
}

#[test]
fn test_serialize_roundtrip() {
    let expr = Expr::Let(X01, Val::Global(0), Box::new(Expr::Fin(X01)));
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let binary = emitter.serialize(&[0], None);

    let prog = encore_vm::program::Program::parse(&binary).unwrap();
    assert_eq!(prog.n_globals(), 1);
    assert_eq!(prog.global(0).raw(), 0);
    assert_eq!(prog.code, [opcode::GLOBAL, X01, 0, opcode::FIN, X01]);
}

#[test]
fn test_extern_stub_and_function() {
    let define_body = Expr::Let(
        X01,
        Val::Extern(0),
        Box::new(Expr::Fin(X01)),
    );
    let mut emitter = Emitter::new();
    emitter.emit_extern_stub(0);
    emitter.emit_toplevel(&define_body);
    let code = emitter.into_bytes();

    // Stub: EXTERN X01, A1, slot=0; MOV A1, X01; ENCORE CONT, NULL
    assert_eq!(code[0], opcode::EXTERN);
    assert_eq!(code[1], 10);      // X01 dest
    assert_eq!(code[2], 2);       // A1 source
    assert_eq!(code[3], 0);       // slot lo
    assert_eq!(code[4], 0);       // slot hi
    assert_eq!(code[5], opcode::MOV);
    assert_eq!(code[6], 2);       // A1
    assert_eq!(code[7], 10);      // X01
    assert_eq!(code[8], opcode::ENCORE);
    assert_eq!(code[9], 1);       // CONT
    assert_eq!(code[10], opcode::NULL);

    // Val::Extern(0) emits FUNCTION pointing to stub at offset 0
    assert_eq!(code[11], opcode::FUNCTION);
    assert_eq!(code[12], X01);    // dest
    assert_eq!(code[13], 0);      // stub addr lo
    assert_eq!(code[14], 0);      // stub addr hi
    // FIN X01
    assert_eq!(code[15], opcode::FIN);
    assert_eq!(code[16], X01);
}
