use encore_vm::opcode;
use crate::ir::{Expr, Lambda, Loc, Val};

pub struct Emitter {
    buf: Vec<u8>,
    arity_table: Vec<u8>,
    deferred: Vec<(usize, *const Expr)>,
}

impl Emitter {
    pub fn new() -> Self {
        Self { buf: Vec::new(), arity_table: Vec::new(), deferred: Vec::new() }
    }

    fn record_arity(&mut self, tag: u8, arity: u8) {
        if tag as usize >= self.arity_table.len() {
            self.arity_table.resize(tag as usize + 1, 0);
        }
        self.arity_table[tag as usize] = arity;
    }

    pub fn arity_table(&self) -> &[u8] {
        &self.arity_table
    }

    pub fn pos(&self) -> usize {
        self.buf.len()
    }

    fn emit_u8(&mut self, byte: u8) {
        self.buf.push(byte);
    }

    fn emit_u16_placeholder(&mut self) -> usize {
        let off = self.pos();
        self.buf.push(0);
        self.buf.push(0);
        off
    }

    fn patch_u16(&mut self, off: usize, val: u16) {
        let bytes = val.to_le_bytes();
        self.buf[off] = bytes[0];
        self.buf[off + 1] = bytes[1];
    }

    pub fn emit_loc(&mut self, loc: &Loc) {
        match loc {
            Loc::Arg => {
                self.emit_u8(opcode::ARG);
            }
            Loc::Local(idx) => {
                self.emit_u8(opcode::LOCAL);
                self.emit_u8(*idx);
            }
            Loc::Capture(idx) => {
                self.emit_u8(opcode::CAPTURE);
                self.emit_u8(*idx);
            }
            Loc::Global(idx) => {
                self.emit_u8(opcode::GLOBAL);
                self.emit_u8(*idx);
            }
            Loc::SelfRef => {
                self.emit_u8(opcode::SELF);
            }
        }
    }

    fn emit_lambda(&mut self, lam: &Lambda) {
        for cap in &lam.captures {
            self.emit_loc(cap);
        }
        self.emit_u8(opcode::CLOSURE);
        let hole = self.emit_u16_placeholder();
        self.emit_u8(lam.captures.len() as u8);
        self.deferred.push((hole, &*lam.body as *const Expr));
    }

    fn emit_val(&mut self, val: &Val) {
        match val {
            Val::Loc(loc) => {
                self.emit_loc(loc);
            }
            Val::Lambda(lam) => {
                self.emit_lambda(lam);
            }
            Val::Ctor(tag, fields) => {
                self.record_arity(*tag, fields.len() as u8);
                for field in fields {
                    self.emit_loc(field);
                }
                self.emit_u8(opcode::PACK);
                self.emit_u8(*tag);
            }
            Val::Field(loc, idx) => {
                self.emit_loc(loc);
                self.emit_u8(opcode::FIELD);
                self.emit_u8(*idx);
            }
        }
    }

    pub fn emit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Let(val, body) => {
                self.emit_val(val);
                self.emit_expr(body);
            }

            Expr::Letrec(lam, body) => {
                self.emit_lambda(lam);
                self.emit_expr(body);
            }

            Expr::App(fun, arg) => {
                self.emit_loc(arg);
                self.emit_loc(fun);
                self.emit_u8(opcode::ENCORE);
            }

            Expr::Match(loc, base, cases) => {
                self.emit_loc(loc);
                self.emit_u8(opcode::MATCH);
                self.emit_u8(*base);
                self.emit_u8(cases.len() as u8);
                let holes: Vec<usize> = (0..cases.len())
                    .map(|_| self.emit_u16_placeholder())
                    .collect();
                for (i, case) in cases.iter().enumerate() {
                    self.patch_u16(holes[i], self.pos() as u16);
                    for f in 0..case.arity {
                        self.emit_u8(opcode::FIELD);
                        self.emit_u8(f);
                    }
                    self.emit_expr(&case.body);
                }
            }

            Expr::Halt(loc) => {
                self.emit_loc(loc);
                self.emit_u8(opcode::HALT);
            }
        }
    }

    pub fn emit_toplevel(&mut self, expr: &Expr) {
        self.emit_expr(expr);
        while let Some((hole, body_ptr)) = self.deferred.pop() {
            self.patch_u16(hole, self.pos() as u16);
            let body = unsafe { &*body_ptr };
            self.emit_expr(body);
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn serialize(self, n_globals: u16) -> Vec<u8> {
        let arity_table = self.arity_table;
        let code = self.buf;
        let mut out = Vec::new();
        out.extend_from_slice(&encore_vm::program::MAGIC);
        out.extend_from_slice(&(arity_table.len() as u16).to_le_bytes());
        out.extend_from_slice(&n_globals.to_le_bytes());
        out.extend_from_slice(&(code.len() as u16).to_le_bytes());
        out.extend_from_slice(&arity_table);
        out.resize(out.len() + n_globals as usize * 4, 0);
        out.extend_from_slice(&code);
        out
    }
}
