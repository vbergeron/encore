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
    extern_stubs: Vec<(u16, u16)>,
}

impl<'a> Emitter<'a> {
    pub fn new() -> Self {
        Self { buf: Vec::new(), arity_table: Vec::new(), deferred: Vec::new(), extern_stubs: Vec::new() }
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

    pub fn emit_extern_stub(&mut self, slot: u16) {
        let addr = self.pos() as u16;
        self.extern_stubs.push((slot, addr));
        self.emit_u8(opcode::NULLADDR);
        self.emit_u8(opcode::ARG);
        self.emit_u8(opcode::EXTERN);
        self.emit_u8(slot as u8);
        self.emit_u8((slot >> 8) as u8);
        self.emit_u8(opcode::CONT);
        self.emit_u8(opcode::ENCORE);
    }

    fn extern_stub_addr(&self, slot: u16) -> u16 {
        self.extern_stubs.iter()
            .find(|(s, _)| *s == slot)
            .expect("extern stub not emitted")
            .1
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
            Loc::NullCont => {
                self.emit_u8(opcode::NULLADDR);
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
        let sd = compute_stack_delta(&fun.body);
        if fun.captures.is_empty() {
            self.emit_u8(opcode::FUNCTION);
            let hole = self.emit_u16_placeholder();
            self.emit_u8(sd);
            self.deferred.push((hole, &fun.body));
        } else {
            for cap in &fun.captures {
                self.emit_loc(cap);
            }
            self.emit_u8(opcode::CLOSURE);
            let hole = self.emit_u16_placeholder();
            self.emit_u8(fun.captures.len() as u8);
            self.emit_u8(sd);
            self.deferred.push((hole, &fun.body));
        }
    }

    fn emit_cont_lam(&mut self, cont: &'a ContLam) {
        let sd = compute_stack_delta(&cont.body);
        if cont.captures.is_empty() {
            self.emit_u8(opcode::FUNCTION);
            let hole = self.emit_u16_placeholder();
            self.emit_u8(sd);
            self.deferred.push((hole, &cont.body));
        } else {
            for cap in &cont.captures {
                self.emit_loc(cap);
            }
            self.emit_u8(opcode::CLOSURE);
            let hole = self.emit_u16_placeholder();
            self.emit_u8(cont.captures.len() as u8);
            self.emit_u8(sd);
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
            Val::Extern(slot) => {
                let addr = self.extern_stub_addr(*slot);
                self.emit_u8(opcode::FUNCTION);
                self.emit_u8(addr as u8);
                self.emit_u8((addr >> 8) as u8);
                self.emit_u8(EXTERN_STUB_SD);
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
        // Return stub at code address 0: acts as the identity continuation
        // for Vm::call — receives result as ARG and halts.
        emitter.emit_u8(opcode::ARG);
        emitter.emit_u8(opcode::FIN);
        let mut extern_slots = Vec::new();
        collect_extern_slots_module(module, &mut extern_slots);
        for slot in &extern_slots {
            emitter.emit_extern_stub(*slot);
        }
        let mut entries: Vec<(u16, u8)> = Vec::with_capacity(module.defines.len());
        for define in &module.defines {
            let sd = compute_stack_delta(&define.body);
            entries.push((emitter.pos() as u16, sd));
            emitter.emit_toplevel(&define.body);
        }
        emitter.serialize(&entries, metadata)
    }

    pub fn serialize(self, entries: &[(u16, u8)], metadata: Option<&Metadata>) -> Vec<u8> {
        let arity_table = self.arity_table;
        let code = self.buf;
        let n_globals = entries.len() as u16;
        let mut out = Vec::new();
        out.extend_from_slice(&encore_vm::program::MAGIC);
        out.extend_from_slice(&(arity_table.len() as u16).to_le_bytes());
        out.extend_from_slice(&n_globals.to_le_bytes());
        out.extend_from_slice(&(code.len() as u16).to_le_bytes());
        out.extend_from_slice(&arity_table);
        for &(addr, sd) in entries {
            out.extend_from_slice(&addr.to_le_bytes());
            out.push(sd);
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

const EXTERN_STUB_SD: u8 = 3;

pub fn compute_stack_delta(expr: &Expr) -> u8 {
    expr_peak(expr, 0) as u8
}

fn expr_peak(expr: &Expr, d: usize) -> usize {
    match expr {
        Expr::Let(val, body) => {
            let vp = val_peak(val, d);
            let bp = expr_peak(body, d + 1);
            vp.max(bp)
        }
        Expr::Letrec(fun, body) => {
            let fp = fun_peak(fun, d);
            let bp = expr_peak(body, d + 1);
            fp.max(bp)
        }
        Expr::Encore(_, _, _) => d + 3,
        Expr::Fin(_) => d + 1,
        Expr::Match(_, _, cases) => {
            let mut peak = d + 1;
            for case in cases {
                let case_depth = if case.arity > 0 {
                    (d + 1).max(d + case.arity as usize)
                } else {
                    d
                };
                let bp = expr_peak(&case.body, case_depth);
                peak = peak.max(case_depth).max(bp);
            }
            peak
        }
    }
}

fn val_peak(val: &Val, d: usize) -> usize {
    match val {
        Val::Loc(_) | Val::Int(_) | Val::Field(_, _) | Val::Extern(_) => d + 1,
        Val::Prim(_, locs) => d + locs.len(),
        Val::Ctor(_, fields) => d + 1.max(fields.len()),
        Val::ContLam(c) => {
            if c.captures.is_empty() { d + 1 } else { d + c.captures.len() }
        }
    }
}

fn fun_peak(fun: &Fun, d: usize) -> usize {
    if fun.captures.is_empty() { d + 1 } else { d + fun.captures.len() }
}

fn collect_extern_slots_module(module: &Module, slots: &mut Vec<u16>) {
    for define in &module.defines {
        collect_extern_slots_expr(&define.body, slots);
    }
    slots.sort();
    slots.dedup();
}

fn collect_extern_slots_expr(expr: &Expr, slots: &mut Vec<u16>) {
    match expr {
        Expr::Let(val, body) => {
            collect_extern_slots_val(val, slots);
            collect_extern_slots_expr(body, slots);
        }
        Expr::Letrec(fun, body) => {
            collect_extern_slots_expr(&fun.body, slots);
            collect_extern_slots_expr(body, slots);
        }
        Expr::Match(_, _, cases) => {
            for case in cases {
                collect_extern_slots_expr(&case.body, slots);
            }
        }
        Expr::Encore(_, _, _) | Expr::Fin(_) => {}
    }
}

fn collect_extern_slots_val(val: &Val, slots: &mut Vec<u16>) {
    match val {
        Val::Extern(slot) => slots.push(*slot),
        Val::ContLam(cont) => collect_extern_slots_expr(&cont.body, slots),
        _ => {}
    }
}
