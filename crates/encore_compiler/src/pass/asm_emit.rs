use encore_vm::opcode;
use crate::ir::asm::{ContLam, Expr, Fun, Loc, Module, Val};
use crate::ir::prim::PrimOp;

pub struct Metadata {
    pub ctor_names: Vec<(u8, String)>,
    pub global_names: Vec<(u8, String)>,
}

pub struct Emitter<'a> {
    buf: Vec<u8>,
    arity_table: Vec<u8>,
    deferred: Vec<(usize, &'a Expr)>,
}

impl<'a> Emitter<'a> {
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
            Loc::Cont => {
                self.emit_u8(opcode::CONT);
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

    fn emit_fun(&mut self, fun: &'a Fun) {
        if fun.captures.is_empty() {
            self.emit_u8(opcode::FUNCTION);
            let hole = self.emit_u16_placeholder();
            self.deferred.push((hole, &fun.body));
        } else {
            for cap in &fun.captures {
                self.emit_loc(cap);
            }
            self.emit_u8(opcode::CLOSURE);
            let hole = self.emit_u16_placeholder();
            self.emit_u8(fun.captures.len() as u8);
            self.deferred.push((hole, &fun.body));
        }
    }

    fn emit_cont_lam(&mut self, cont: &'a ContLam) {
        if cont.captures.is_empty() {
            self.emit_u8(opcode::FUNCTION);
            let hole = self.emit_u16_placeholder();
            self.deferred.push((hole, &cont.body));
        } else {
            for cap in &cont.captures {
                self.emit_loc(cap);
            }
            self.emit_u8(opcode::CLOSURE);
            let hole = self.emit_u16_placeholder();
            self.emit_u8(cont.captures.len() as u8);
            self.deferred.push((hole, &cont.body));
        }
    }

    fn emit_val(&mut self, val: &'a Val) {
        match val {
            Val::Loc(loc) => {
                self.emit_loc(loc);
            }
            Val::ContLam(cont) => {
                self.emit_cont_lam(cont);
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
            Val::Int(n) => {
                match *n {
                    0 => self.emit_u8(opcode::INT_0),
                    1 => self.emit_u8(opcode::INT_1),
                    2 => self.emit_u8(opcode::INT_2),
                    _ => {
                        self.emit_u8(opcode::INT);
                        let bits = *n as u32;
                        self.emit_u8(bits as u8);
                        self.emit_u8((bits >> 8) as u8);
                        self.emit_u8((bits >> 16) as u8);
                    }
                }
            }
            Val::Prim(op, locs) => {
                for loc in locs {
                    self.emit_loc(loc);
                }
                match op {
                    PrimOp::Add => self.emit_u8(opcode::INT_ADD),
                    PrimOp::Sub => self.emit_u8(opcode::INT_SUB),
                    PrimOp::Mul => self.emit_u8(opcode::INT_MUL),
                    PrimOp::Eq  => self.emit_u8(opcode::INT_EQ),
                    PrimOp::Lt  => self.emit_u8(opcode::INT_LT),
                }
            }
        }
    }

    pub fn emit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Let(val, body) => {
                self.emit_val(val);
                self.emit_expr(body);
            }

            Expr::Letrec(fun, body) => {
                self.emit_fun(fun);
                self.emit_expr(body);
            }

            Expr::Encore(fun, arg, cont) => {
                self.emit_loc(cont);
                self.emit_loc(arg);
                self.emit_loc(fun);
                self.emit_u8(opcode::ENCORE);
            }

            Expr::Return(cont, result) => {
                self.emit_loc(result);
                self.emit_loc(cont);
                self.emit_u8(opcode::RETURN);
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
                    if case.arity > 0 {
                        self.emit_loc(loc);
                        self.emit_u8(opcode::UNPACK);
                        self.emit_u8(base + i as u8);
                    }
                    self.emit_expr(&case.body);
                }
            }

            Expr::Fin(loc) => {
                self.emit_loc(loc);
                self.emit_u8(opcode::FIN);
            }
        }
    }

    pub fn emit_toplevel(&mut self, expr: &'a Expr) {
        self.emit_expr(expr);
        while let Some((hole, body)) = self.deferred.pop() {
            self.patch_u16(hole, self.pos() as u16);
            self.emit_expr(body);
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    pub fn emit_module(module: &Module, metadata: Option<&Metadata>) -> Vec<u8> {
        let mut emitter = Self::new();
        let mut entry_addrs = Vec::with_capacity(module.defines.len());
        for define in &module.defines {
            entry_addrs.push(emitter.pos() as u16);
            emitter.emit_toplevel(&define.body);
        }
        emitter.serialize(&entry_addrs, metadata)
    }

    pub fn serialize(self, entry_addrs: &[u16], metadata: Option<&Metadata>) -> Vec<u8> {
        let arity_table = self.arity_table;
        let code = self.buf;
        let n_globals = entry_addrs.len() as u16;
        let mut out = Vec::new();
        out.extend_from_slice(&encore_vm::program::MAGIC);
        out.extend_from_slice(&(arity_table.len() as u16).to_le_bytes());
        out.extend_from_slice(&n_globals.to_le_bytes());
        out.extend_from_slice(&(code.len() as u16).to_le_bytes());
        out.extend_from_slice(&arity_table);
        for &addr in entry_addrs {
            out.extend_from_slice(&addr.to_le_bytes());
        }
        out.extend_from_slice(&code);
        if let Some(meta) = metadata {
            serialize_name_section(&mut out, &meta.ctor_names);
            serialize_name_section(&mut out, &meta.global_names);
        }
        out
    }
}

fn serialize_name_section(out: &mut Vec<u8>, entries: &[(u8, String)]) {
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (idx, name) in entries {
        out.push(*idx);
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
    }
}
