use encore_compiler::pass::asm_emit::Emitter;
use encore_compiler::ir::asm::*;
use encore_vm::opcode;

#[test]
fn test_fin_global() {
    let expr = Expr::Fin(Loc::Global(0));
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();

    assert_eq!(code, [opcode::GLOBAL, 0, opcode::FIN]);
}

#[test]
fn test_fin_arg() {
    let expr = Expr::Fin(Loc::Arg);
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();

    assert_eq!(code, [opcode::ARG, opcode::FIN]);
}

#[test]
fn test_fin_capture() {
    let expr = Expr::Fin(Loc::Capture(2));
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();

    assert_eq!(code, [opcode::CAPTURE, 2, opcode::FIN]);
}

#[test]
fn test_match_two_branches() {
    let expr = Expr::Match(
        Loc::Global(0),
        0,
        vec![
            Case { arity: 0, body: Expr::Fin(Loc::Global(1)) },
            Case { arity: 0, body: Expr::Fin(Loc::Global(2)) },
        ],
    );
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();

    assert_eq!(code[0], opcode::GLOBAL);
    assert_eq!(code[1], 0);
    assert_eq!(code[2], opcode::MATCH);
    assert_eq!(code[3], 0); // base tag
    assert_eq!(code[4], 2); // n branches
    let off0 = u16::from_le_bytes([code[5], code[6]]);
    let off1 = u16::from_le_bytes([code[7], code[8]]);
    // branch 0 body
    assert_eq!(code[off0 as usize], opcode::GLOBAL);
    assert_eq!(code[off0 as usize + 1], 1);
    assert_eq!(code[off0 as usize + 2], opcode::FIN);
    // branch 1 body
    assert_eq!(code[off1 as usize], opcode::GLOBAL);
    assert_eq!(code[off1 as usize + 1], 2);
    assert_eq!(code[off1 as usize + 2], opcode::FIN);
}

#[test]
fn test_letrec_deferred_body() {
    let expr = Expr::Letrec(
        Fun {
            captures: vec![Loc::Global(0)],
            body: Box::new(Expr::Fin(Loc::Capture(0))),
        },
        Box::new(Expr::Fin(Loc::Global(1))),
    );

    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();

    assert_eq!(code[0], opcode::GLOBAL);
    assert_eq!(code[1], 0);
    assert_eq!(code[2], opcode::CLOSURE);
    let body_addr = u16::from_le_bytes([code[3], code[4]]);
    assert_eq!(code[5], 1); // ncap
    assert_eq!(code[6], 1); // sd
    assert_eq!(code[7], opcode::GLOBAL);
    assert_eq!(code[8], 1);
    assert_eq!(code[9], opcode::FIN);
    assert_eq!(body_addr, 10);
    assert_eq!(code[10], opcode::CAPTURE);
    assert_eq!(code[11], 0);
    assert_eq!(code[12], opcode::FIN);
}

#[test]
fn test_emit_expr_no_fin() {
    let body = Expr::Fin(Loc::Global(0));
    let mut emitter = Emitter::new();
    emitter.emit_expr(&body);
    let code = emitter.into_bytes();

    assert_eq!(code, [opcode::GLOBAL, 0, opcode::FIN]);
}

#[test]
fn test_serialize_roundtrip() {
    let expr = Expr::Fin(Loc::Global(0));
    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let binary = emitter.serialize(&[(0, 2)], None);

    let prog = encore_vm::program::Program::parse(&binary).unwrap();
    assert_eq!(prog.n_globals(), 1);
    assert_eq!(prog.global(0).raw(), 0);
    assert_eq!(prog.code, [opcode::GLOBAL, 0, opcode::FIN]);
}

#[test]
fn test_extern_stub_and_function() {
    let define_body = Expr::Let(
        Val::Extern(0),
        Box::new(Expr::Fin(Loc::Local(0))),
    );
    let mut emitter = Emitter::new();
    emitter.emit_extern_stub(0);
    emitter.emit_toplevel(&define_body);
    let code = emitter.into_bytes();

    // stub: NULLADDR, ARG, EXTERN 0x00 0x00, CONT, ENCORE = 7 bytes at offset 0
    assert_eq!(code[0], opcode::NULLADDR);
    assert_eq!(code[1], opcode::ARG);
    assert_eq!(code[2], opcode::EXTERN);
    assert_eq!(code[3], 0); // slot lo
    assert_eq!(code[4], 0); // slot hi
    assert_eq!(code[5], opcode::CONT);
    assert_eq!(code[6], opcode::ENCORE);
    // Val::Extern(0) emits FUNCTION pointing to stub at offset 0
    assert_eq!(code[7], opcode::FUNCTION);
    assert_eq!(code[8], 0); // stub addr lo
    assert_eq!(code[9], 0); // stub addr hi
    assert_eq!(code[10], 3); // sd for extern stub
}
