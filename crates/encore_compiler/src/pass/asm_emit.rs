use encore_vm::opcode;
use crate::ir::asm::{ContLam, Expr, Fun, Module, Reg, Val};
use crate::ir::prim::{PrimOp, IntOp, BytesOp};

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
        // EXTERN X01, A1, slot
        self.emit_u8(opcode::EXTERN);
        self.emit_u8(10); // X01
        self.emit_u8(2); // A1
        self.emit_u8(slot as u8);
        self.emit_u8((slot >> 8) as u8);
        // MOV A1, X01; ENCORE CONT, NULL
        self.emit_u8(opcode::MOV);
        self.emit_u8(2); // A1
        self.emit_u8(10); // X01
        self.emit_u8(opcode::ENCORE);
        self.emit_u8(1); // CONT
        self.emit_u8(opcode::NULL);
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

    fn emit_fun(&mut self, dest: Reg, fun: &'a Fun) {
        if fun.captures.is_empty() {
            self.emit_u8(opcode::FUNCTION);
            self.emit_u8(dest);
            let hole = self.emit_u16_placeholder();
            self.deferred.push((hole, &fun.body));
        } else {
            self.emit_u8(opcode::CLOSURE);
            self.emit_u8(dest);
            let hole = self.emit_u16_placeholder();
            self.emit_u8(fun.captures.len() as u8);
            for cap in &fun.captures {
                self.emit_u8(*cap);
            }
            self.deferred.push((hole, &fun.body));
        }
    }

    fn emit_cont_lam(&mut self, dest: Reg, cont: &'a ContLam) {
        if cont.captures.is_empty() {
            self.emit_u8(opcode::FUNCTION);
            self.emit_u8(dest);
            let hole = self.emit_u16_placeholder();
            self.deferred.push((hole, &cont.body));
        } else {
            self.emit_u8(opcode::CLOSURE);
            self.emit_u8(dest);
            let hole = self.emit_u16_placeholder();
            self.emit_u8(cont.captures.len() as u8);
            for cap in &cont.captures {
                self.emit_u8(*cap);
            }
            self.deferred.push((hole, &cont.body));
        }
    }

    fn emit_val(&mut self, dest: Reg, val: &'a Val) {
        match val {
            Val::Reg(src) => {
                self.emit_u8(opcode::MOV);
                self.emit_u8(dest);
                self.emit_u8(*src);
            }
            Val::Capture(idx) => {
                self.emit_u8(opcode::CAPTURE);
                self.emit_u8(dest);
                self.emit_u8(*idx);
            }
            Val::Global(idx) => {
                self.emit_u8(opcode::GLOBAL);
                self.emit_u8(dest);
                self.emit_u8(*idx);
            }
            Val::ContLam(cont) => {
                self.emit_cont_lam(dest, cont);
            }
            Val::Ctor(tag, fields) => {
                self.record_arity(*tag, fields.len() as u8);
                self.emit_u8(opcode::PACK);
                self.emit_u8(dest);
                self.emit_u8(*tag);
                for field in fields {
                    self.emit_u8(*field);
                }
            }
            Val::Field(src, idx) => {
                self.emit_u8(opcode::FIELD);
                self.emit_u8(dest);
                self.emit_u8(*src);
                self.emit_u8(*idx);
            }
            Val::Int(n) => {
                match *n {
                    0 => {
                        self.emit_u8(opcode::INT_0);
                        self.emit_u8(dest);
                    }
                    1 => {
                        self.emit_u8(opcode::INT_1);
                        self.emit_u8(dest);
                    }
                    2 => {
                        self.emit_u8(opcode::INT_2);
                        self.emit_u8(dest);
                    }
                    _ => {
                        self.emit_u8(opcode::INT);
                        self.emit_u8(dest);
                        let bits = *n as u32;
                        self.emit_u8(bits as u8);
                        self.emit_u8((bits >> 8) as u8);
                        self.emit_u8((bits >> 16) as u8);
                    }
                }
            }
            Val::Bytes(data) => {
                self.emit_u8(opcode::BYTES);
                self.emit_u8(dest);
                self.emit_u8(data.len() as u8);
                for &b in data {
                    self.emit_u8(b);
                }
            }
            Val::Prim(op, regs) => {
                let opc = match op {
                    PrimOp::Int(IntOp::Add) => opcode::INT_ADD,
                    PrimOp::Int(IntOp::Sub) => opcode::INT_SUB,
                    PrimOp::Int(IntOp::Mul) => opcode::INT_MUL,
                    PrimOp::Int(IntOp::Eq)  => opcode::INT_EQ,
                    PrimOp::Int(IntOp::Lt)   => opcode::INT_LT,
                    PrimOp::Int(IntOp::Byte) => opcode::INT_BYTE,
                    PrimOp::Bytes(BytesOp::Len)    => opcode::BYTES_LEN,
                    PrimOp::Bytes(BytesOp::Get)    => opcode::BYTES_GET,
                    PrimOp::Bytes(BytesOp::Concat) => opcode::BYTES_CONCAT,
                    PrimOp::Bytes(BytesOp::Slice)  => opcode::BYTES_SLICE,
                    PrimOp::Bytes(BytesOp::Eq)     => opcode::BYTES_EQ,
                };
                self.emit_u8(opc);
                self.emit_u8(dest);
                for r in regs {
                    self.emit_u8(*r);
                }
            }
            Val::Extern(slot) => {
                let addr = self.extern_stub_addr(*slot);
                self.emit_u8(opcode::FUNCTION);
                self.emit_u8(dest);
                self.emit_u8(addr as u8);
                self.emit_u8((addr >> 8) as u8);
            }
        }
    }

    pub fn emit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Let(dest, val, body) => {
                self.emit_val(*dest, val);
                self.emit_expr(body);
            }

            Expr::Letrec(dest, fun, body) => {
                self.emit_fun(*dest, fun);
                self.emit_expr(body);
            }

            Expr::Encore(fun, args, cont) => {
                for (i, arg) in args.iter().enumerate() {
                    let ai = 2 + i as u8; // A1 = 2, A2 = 3, ...
                    if *arg != ai {
                        self.emit_u8(opcode::MOV);
                        self.emit_u8(ai);
                        self.emit_u8(*arg);
                    }
                }
                self.emit_u8(opcode::ENCORE);
                self.emit_u8(*fun);
                self.emit_u8(*cont);
            }

            Expr::Match(reg, base, cases) if cases.len() == 2 => {
                self.emit_u8(opcode::BRANCH);
                self.emit_u8(*reg);
                self.emit_u8(*base);
                let hole0 = self.emit_u16_placeholder();
                let hole1 = self.emit_u16_placeholder();
                self.patch_u16(hole0, self.pos() as u16);
                if cases[0].arity > 0 {
                    self.record_arity(*base, cases[0].arity);
                    self.emit_u8(opcode::UNPACK);
                    self.emit_u8(cases[0].unpack_base);
                    self.emit_u8(*base);
                    self.emit_u8(*reg);
                }
                self.emit_expr(&cases[0].body);
                self.patch_u16(hole1, self.pos() as u16);
                if cases[1].arity > 0 {
                    self.record_arity(*base + 1, cases[1].arity);
                    self.emit_u8(opcode::UNPACK);
                    self.emit_u8(cases[1].unpack_base);
                    self.emit_u8(*base + 1);
                    self.emit_u8(*reg);
                }
                self.emit_expr(&cases[1].body);
            }

            Expr::Match(reg, base, cases) => {
                self.emit_u8(opcode::MATCH);
                self.emit_u8(*reg);
                self.emit_u8(*base);
                self.emit_u8(cases.len() as u8);
                let holes: Vec<usize> = (0..cases.len())
                    .map(|_| self.emit_u16_placeholder())
                    .collect();
                for (i, case) in cases.iter().enumerate() {
                    self.patch_u16(holes[i], self.pos() as u16);
                    if case.arity > 0 {
                        self.record_arity(base + i as u8, case.arity);
                        self.emit_u8(opcode::UNPACK);
                        self.emit_u8(case.unpack_base);
                        self.emit_u8(base + i as u8);
                        self.emit_u8(*reg);
                    }
                    self.emit_expr(&case.body);
                }
            }

            Expr::Fin(reg) => {
                self.emit_u8(opcode::FIN);
                self.emit_u8(*reg);
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
        // Return stub at code address 0: FIN A1
        emitter.emit_u8(opcode::FIN);
        emitter.emit_u8(2); // A1 register
        let mut extern_slots = Vec::new();
        collect_extern_slots_module(module, &mut extern_slots);
        for slot in &extern_slots {
            emitter.emit_extern_stub(*slot);
        }
        let mut entries: Vec<u16> = Vec::with_capacity(module.defines.len());
        for define in &module.defines {
            entries.push(emitter.pos() as u16);
            emitter.emit_toplevel(&define.body);
        }
        emitter.serialize(&entries, metadata)
    }

    pub fn serialize(self, entries: &[u16], metadata: Option<&Metadata>) -> Vec<u8> {
        let arity_table = self.arity_table;
        let code = self.buf;
        let n_globals = entries.len() as u16;
        let mut out = Vec::new();
        out.extend_from_slice(&encore_vm::program::MAGIC);
        out.extend_from_slice(&(arity_table.len() as u16).to_le_bytes());
        out.extend_from_slice(&n_globals.to_le_bytes());
        out.extend_from_slice(&(code.len() as u16).to_le_bytes());
        out.extend_from_slice(&arity_table);
        for &addr in entries {
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

fn collect_extern_slots_module(module: &Module, slots: &mut Vec<u16>) {
    for define in &module.defines {
        collect_extern_slots_expr(&define.body, slots);
    }
    slots.sort();
    slots.dedup();
}

fn collect_extern_slots_expr(expr: &Expr, slots: &mut Vec<u16>) {
    match expr {
        Expr::Let(_, val, body) => {
            collect_extern_slots_val(val, slots);
            collect_extern_slots_expr(body, slots);
        }
        Expr::Letrec(_, fun, body) => {
            collect_extern_slots_expr(&fun.body, slots);
            collect_extern_slots_expr(body, slots);
        }
        Expr::Match(_, _, cases) => {
            for case in cases {
                collect_extern_slots_expr(&case.body, slots);
            }
        }
        Expr::Encore(_, _, _) | Expr::Fin(_) => {},
    }
}

fn collect_extern_slots_val(val: &Val, slots: &mut Vec<u16>) {
    match val {
        Val::Extern(slot) => slots.push(*slot),
        Val::ContLam(cont) => collect_extern_slots_expr(&cont.body, slots),
        _ => {}
    }
}
