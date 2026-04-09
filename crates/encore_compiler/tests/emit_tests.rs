use encore_compiler::pass::emit::Emitter;
use encore_compiler::ir::asm::*;
use encore_vm::program::Program;
use encore_vm::value::{CodeAddress, HeapAddress, Value};
use encore_vm::vm::Vm;

// -- Test 1: Fin(Global(0)) via run() --

#[test]
fn test_halt_global() {
    let expr = Expr::Fin(Loc::Global(0));

    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let binary = emitter.serialize(1);
    let prog = Program::parse(&binary).unwrap();

    let tag7 = Value::ctor(7, HeapAddress::NULL);
    let globals = [tag7];
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(prog.code, prog.arity_table, &globals, &mut mem);
    let result = vm.run().unwrap();
    assert_eq!(result.to_u32(), tag7.to_u32());
}

// -- Test 2: Emit a function body, call via Vm::call --

#[test]
fn test_call_lambda_body() {
    let body = Expr::Fin(Loc::Global(0));

    let mut emitter = Emitter::new();
    emitter.emit_expr(&body);
    let binary = emitter.serialize(1);
    let prog = Program::parse(&binary).unwrap();

    let tag5 = Value::ctor(5, HeapAddress::NULL);
    let dummy_arg = Value::ctor(0, HeapAddress::NULL);
    let globals = [tag5];
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(prog.code, prog.arity_table, &globals, &mut mem);
    let result = vm.call(CodeAddress::new(0), dummy_arg).unwrap();
    assert_eq!(result.to_u32(), tag5.to_u32());
}

// -- Test 3: Match on constructor tags --

#[test]
fn test_match_branch0() {
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
    let binary = emitter.serialize(3);
    let prog = Program::parse(&binary).unwrap();

    let g0 = Value::ctor(0, HeapAddress::NULL);
    let g1 = Value::ctor(10, HeapAddress::NULL);
    let g2 = Value::ctor(20, HeapAddress::NULL);
    let globals = [g0, g1, g2];
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(prog.code, prog.arity_table, &globals, &mut mem);
    let result = vm.run().unwrap();
    assert_eq!(result.to_u32(), g1.to_u32());
}

#[test]
fn test_match_branch1() {
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
    let binary = emitter.serialize(3);
    let prog = Program::parse(&binary).unwrap();

    let g0 = Value::ctor(1, HeapAddress::NULL);
    let g1 = Value::ctor(10, HeapAddress::NULL);
    let g2 = Value::ctor(20, HeapAddress::NULL);
    let globals = [g0, g1, g2];
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(prog.code, prog.arity_table, &globals, &mut mem);
    let result = vm.run().unwrap();
    assert_eq!(result.to_u32(), g2.to_u32());
}

// -- Test 4: Lambda deferred body + Letrec --

#[test]
fn test_letrec_deferred_body() {
    let expr = Expr::Letrec(
        Lambda {
            captures: vec![Loc::Global(0)],
            body: Box::new(Expr::Fin(Loc::Capture(0))),
        },
        Box::new(Expr::Fin(Loc::Global(1))),
    );

    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let code = emitter.into_bytes();

    use encore_vm::opcode;
    // Expected bytecode:
    // 0: GLOBAL 0             (push Global(0) as capture)
    // 2: CLOSURE <addr> 1     (ncap=1, body deferred)
    // 6: GLOBAL 1             (push Global(1))
    // 8: FIN
    // 9: CAPTURE 0            (deferred body: push Capture(0))
    // 11: FIN
    assert_eq!(code[0], opcode::GLOBAL);
    assert_eq!(code[1], 0);
    assert_eq!(code[2], opcode::CLOSURE);
    let body_addr = u16::from_le_bytes([code[3], code[4]]);
    assert_eq!(code[5], 1);
    assert_eq!(code[6], opcode::GLOBAL);
    assert_eq!(code[7], 1);
    assert_eq!(code[8], opcode::FIN);
    assert_eq!(body_addr, 9);
    assert_eq!(code[9], opcode::CAPTURE);
    assert_eq!(code[10], 0);
    assert_eq!(code[11], opcode::FIN);
}

#[test]
fn test_letrec_run() {
    let expr = Expr::Letrec(
        Lambda {
            captures: vec![Loc::Global(0)],
            body: Box::new(Expr::Fin(Loc::Capture(0))),
        },
        Box::new(Expr::Fin(Loc::Global(1))),
    );

    let mut emitter = Emitter::new();
    emitter.emit_toplevel(&expr);
    let binary = emitter.serialize(2);
    let prog = Program::parse(&binary).unwrap();

    let g0 = Value::ctor(0, HeapAddress::NULL);
    let g1 = Value::ctor(1, HeapAddress::NULL);
    let globals = [g0, g1];
    let mut mem = [Value::from_u32(0); 1024];
    let mut vm = Vm::new(prog.code, prog.arity_table, &globals, &mut mem);
    let result = vm.run().unwrap();
    assert_eq!(result.to_u32(), g1.to_u32());
}
