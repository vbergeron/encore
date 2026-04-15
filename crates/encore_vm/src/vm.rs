use crate::arena::Arena;
use crate::code::Code;
use crate::error::VmError;
use crate::opcode;
use crate::program::Program;
#[cfg(feature = "stats")]
use crate::stats::VmStats;
use crate::value::{CodeAddress, HeapAddress, Value};

const SELF_REF: usize = 0;
const CONT: usize = 1;
const A1: usize = 2;
const N_REGS: usize = 32;

pub type ExternFn = fn(Value) -> Value;
const MAX_EXTERN: usize = 32;

pub struct Vm<'a> {
    code: Code<'a>,
    arity_table: &'a [u8],
    globals: [Value; 64],
    n_globals: u8,
    extern_fns: [Option<ExternFn>; MAX_EXTERN],
    arena: Arena<'a>,
    regs: [Value; N_REGS],
    #[cfg(feature = "stats")]
    stats: VmStats,
}

impl<'a> Vm<'a> {
    pub fn init(mem: &'a mut [Value]) -> Self {
        Self {
            code: Code::new(&[]),
            arity_table: &[],
            globals: [Value::from_u32(0); 64],
            n_globals: 0,
            extern_fns: [None; MAX_EXTERN],
            arena: Arena::new(mem),
            regs: [Value::from_u32(0); N_REGS],
            #[cfg(feature = "stats")]
            stats: VmStats::default(),
        }
    }

    pub fn register_extern(&mut self, slot: u16, f: ExternFn) {
        self.extern_fns[slot as usize] = Some(f);
    }

    pub fn load(&mut self, prog: &'a Program) -> Result<(), VmError> {
        self.code = Code::new(prog.code);
        self.arity_table = prog.arity_table;
        self.n_globals = prog.n_globals() as u8;
        for i in 0..self.n_globals as usize {
            let addr = prog.global(i);
            self.globals[i] = self.call_address(addr, Value::from_u32(0))?;
        }
        Ok(())
    }

    pub fn global(&self, idx: usize) -> Value {
        self.globals[idx]
    }

    pub fn ctor_field(&self, val: Value, idx: usize) -> Value {
        self.arena.heap_read(val.ctor_addr(), 1 + idx)
    }

    pub fn alloc_bytes(&mut self, data: &[u8]) -> Result<Value, VmError> {
        let n_data_words = (data.len() + 3) / 4;
        let total = 2 + n_data_words;
        let addr = self.alloc(total)?;
        self.arena.heap_write(addr, 0, Value::gc_header(total as u8));
        self.arena.heap_write(addr, 1, Value::bytes_header(data.len()));
        for i in 0..n_data_words {
            let base = i * 4;
            let mut word: u32 = 0;
            for j in 0..4 {
                if base + j < data.len() {
                    word |= (data[base + j] as u32) << (j * 8);
                }
            }
            self.arena.heap_write(addr, 2 + i, Value::from_u32(word));
        }
        Ok(Value::bytes(addr))
    }

    pub fn bytes_len(&self, val: Value) -> usize {
        self.arena.heap_read(val.bytes_addr(), 1).bytes_hdr_len()
    }

    pub fn bytes_read(&self, val: Value, idx: usize) -> u8 {
        let word_idx = idx / 4;
        let byte_off = idx % 4;
        let word = self.arena.heap_read(val.bytes_addr(), 2 + word_idx).to_u32();
        (word >> (byte_off * 8)) as u8
    }

    fn read_reg(&self, r: u8) -> Value {
        if r == opcode::NULL {
            Value::from_u32(0xFFFF)
        } else {
            self.regs[r as usize]
        }
    }

    fn alloc(&mut self, n: usize) -> Result<HeapAddress, VmError> {
        self.arena.alloc(n, &mut self.regs, &mut self.globals[..self.n_globals as usize])
    }

    fn resolve_code_ptr(&self, func: Value) -> CodeAddress {
        if func.is_function() {
            func.code_ptr()
        } else {
            self.arena.heap_read(func.closure_addr(), 1).header_code_ptr()
        }
    }

    const RETURN_CONT: Value = Value::function_const(0);

    pub fn call(&mut self, global_idx: usize, arg: Value) -> Result<Value, VmError> {
        let func = self.globals[global_idx];
        let code_ptr = self.resolve_code_ptr(func);
        self.regs[SELF_REF] = func;
        self.regs[A1] = arg;
        self.regs[CONT] = Self::RETURN_CONT;
        self.code.jump(code_ptr);
        self.run()
    }

    pub fn call_value(&mut self, func: Value, arg: Value) -> Result<Value, VmError> {
        let code_ptr = self.resolve_code_ptr(func);
        self.regs[SELF_REF] = func;
        self.regs[A1] = arg;
        self.regs[CONT] = Self::RETURN_CONT;
        self.code.jump(code_ptr);
        self.run()
    }

    fn call_address(&mut self, entry: CodeAddress, arg: Value) -> Result<Value, VmError> {
        self.regs[SELF_REF] = Value::function(entry);
        self.regs[A1] = arg;
        self.regs[CONT] = Self::RETURN_CONT;
        self.code.jump(entry);
        self.run()
    }

    #[cfg(feature = "stats")]
    pub fn stats(&self) -> VmStats {
        VmStats {
            op_count: self.stats.op_count,
            arena: self.arena.stats,
        }
    }

    fn run(&mut self) -> Result<Value, VmError> {
        loop {
            #[cfg(feature = "stats")]
            { self.stats.op_count += 1; }
            let op = self.code.read_u8();
            match op {
                opcode::MOV => {
                    let rd = self.code.read_u8() as usize;
                    let rs = self.code.read_u8();
                    self.regs[rd] = self.read_reg(rs);
                }

                opcode::CAPTURE => {
                    let rd = self.code.read_u8() as usize;
                    let idx = self.code.read_u8() as usize;
                    let addr = self.regs[SELF_REF].closure_addr();
                    self.regs[rd] = self.arena.heap_read(addr, 2 + idx);
                }

                opcode::GLOBAL => {
                    let rd = self.code.read_u8() as usize;
                    let idx = self.code.read_u8() as usize;
                    self.regs[rd] = self.globals[idx];
                }

                opcode::CLOSURE => {
                    let rd = self.code.read_u8() as usize;
                    let code_ptr = self.code.read_address();
                    let ncap = self.code.read_u8();
                    let size = 2 + ncap as usize;
                    let mut caps = [0u8; 17];
                    for i in 0..ncap as usize {
                        caps[i] = self.code.read_u8();
                    }
                    let addr = self.alloc(size)?;
                    self.arena.heap_write(addr, 0, Value::gc_header(size as u8));
                    self.arena.heap_write(addr, 1, Value::closure_header(ncap, code_ptr));
                    for i in 0..ncap as usize {
                        self.arena.heap_write(addr, 2 + i, self.read_reg(caps[i]));
                    }
                    self.regs[rd] = Value::closure(addr);
                }

                opcode::FUNCTION => {
                    let rd = self.code.read_u8() as usize;
                    let code_ptr = self.code.read_address();
                    self.regs[rd] = Value::function(code_ptr);
                }

                opcode::PACK => {
                    let rd = self.code.read_u8() as usize;
                    let tag = self.code.read_u8();
                    let arity = self.arity_table[tag as usize] as usize;
                    if arity == 0 {
                        self.regs[rd] = Value::ctor(tag, HeapAddress::NULL);
                    } else {
                        let mut fields = [0u8; 17];
                        for i in 0..arity {
                            fields[i] = self.code.read_u8();
                        }
                        let size = 1 + arity;
                        let addr = self.alloc(size)?;
                        self.arena.heap_write(addr, 0, Value::gc_header(size as u8));
                        for i in 0..arity {
                            self.arena.heap_write(addr, 1 + i, self.read_reg(fields[i]));
                        }
                        self.regs[rd] = Value::ctor(tag, addr);
                    }
                }

                opcode::FIELD => {
                    let rd = self.code.read_u8() as usize;
                    let rs = self.code.read_u8();
                    let idx = self.code.read_u8() as usize;
                    let ctor = self.read_reg(rs);
                    self.regs[rd] = self.arena.heap_read(ctor.ctor_addr(), 1 + idx);
                }

                opcode::UNPACK => {
                    let rd = self.code.read_u8() as usize;
                    let tag = self.code.read_u8();
                    let rs = self.code.read_u8();
                    let arity = self.arity_table[tag as usize] as usize;
                    let ctor = self.read_reg(rs);
                    let addr = ctor.ctor_addr();
                    for i in 0..arity {
                        self.regs[rd + i] = self.arena.heap_read(addr, 1 + i);
                    }
                }

                opcode::MATCH => {
                    let rs = self.code.read_u8();
                    let base = self.code.read_u8();
                    let n = self.code.read_u8();
                    let table = self.code.pc();
                    self.code.skip(n as usize * 2);
                    let ctor = self.read_reg(rs);
                    let branch = ctor.ctor_tag().wrapping_sub(base) as usize;
                    if branch >= n as usize {
                        return Err(VmError::MatchFail);
                    }
                    let off = self.code.read_address_at(table + branch * 2);
                    self.code.jump(off);
                }

                opcode::ENCORE => {
                    let rf = self.code.read_u8();
                    let rk = self.code.read_u8();
                    let fun = self.read_reg(rf);
                    let cont = self.read_reg(rk);
                    let code_ptr = self.resolve_code_ptr(fun);
                    self.regs[SELF_REF] = fun;
                    self.regs[CONT] = cont;
                    self.code.jump(code_ptr);
                }

                opcode::INT => {
                    let rd = self.code.read_u8() as usize;
                    let b0 = self.code.read_u8() as u32;
                    let b1 = self.code.read_u8() as u32;
                    let b2 = self.code.read_u8() as u32;
                    let raw = b0 | (b1 << 8) | (b2 << 16);
                    let n = ((raw as i32) << 8) >> 8;
                    self.regs[rd] = Value::int(n);
                }

                opcode::INT_0 => {
                    let rd = self.code.read_u8() as usize;
                    self.regs[rd] = Value::int(0);
                }

                opcode::INT_1 => {
                    let rd = self.code.read_u8() as usize;
                    self.regs[rd] = Value::int(1);
                }

                opcode::INT_2 => {
                    let rd = self.code.read_u8() as usize;
                    self.regs[rd] = Value::int(2);
                }

                opcode::INT_ADD => {
                    let rd = self.code.read_u8() as usize;
                    let ra = self.code.read_u8();
                    let rb = self.code.read_u8();
                    let a = self.read_reg(ra).int_value();
                    let b = self.read_reg(rb).int_value();
                    self.regs[rd] = Value::int(a.wrapping_add(b));
                }

                opcode::INT_SUB => {
                    let rd = self.code.read_u8() as usize;
                    let ra = self.code.read_u8();
                    let rb = self.code.read_u8();
                    let a = self.read_reg(ra).int_value();
                    let b = self.read_reg(rb).int_value();
                    self.regs[rd] = Value::int(a.wrapping_sub(b));
                }

                opcode::INT_MUL => {
                    let rd = self.code.read_u8() as usize;
                    let ra = self.code.read_u8();
                    let rb = self.code.read_u8();
                    let a = self.read_reg(ra).int_value();
                    let b = self.read_reg(rb).int_value();
                    self.regs[rd] = Value::int(a.wrapping_mul(b));
                }

                opcode::INT_EQ => {
                    let rd = self.code.read_u8() as usize;
                    let ra = self.code.read_u8();
                    let rb = self.code.read_u8();
                    let a = self.read_reg(ra).int_value();
                    let b = self.read_reg(rb).int_value();
                    let tag = if a == b { 1 } else { 0 };
                    self.regs[rd] = Value::ctor(tag, HeapAddress::NULL);
                }

                opcode::INT_LT => {
                    let rd = self.code.read_u8() as usize;
                    let ra = self.code.read_u8();
                    let rb = self.code.read_u8();
                    let a = self.read_reg(ra).int_value();
                    let b = self.read_reg(rb).int_value();
                    let tag = if a < b { 1 } else { 0 };
                    self.regs[rd] = Value::ctor(tag, HeapAddress::NULL);
                }

                opcode::INT_BYTE => {
                    let rd = self.code.read_u8() as usize;
                    let rs = self.code.read_u8();
                    let n = self.read_reg(rs).int_value();
                    if n < 0 || n > 255 {
                        return Err(VmError::ByteRange(n));
                    }
                    let addr = self.alloc(3)?;
                    self.arena.heap_write(addr, 0, Value::gc_header(3));
                    self.arena.heap_write(addr, 1, Value::bytes_header(1));
                    self.arena.heap_write(addr, 2, Value::from_u32(n as u32));
                    self.regs[rd] = Value::bytes(addr);
                }

                opcode::EXTERN => {
                    let rd = self.code.read_u8() as usize;
                    let idx = self.code.read_u16();
                    let ra = self.code.read_u8();
                    let arg = self.read_reg(ra);
                    let f = self.extern_fns[idx as usize]
                        .ok_or(VmError::NotRegistered(idx))?;
                    let result = f(arg);
                    self.regs[rd] = result;
                }

                opcode::BYTES => {
                    let rd = self.code.read_u8() as usize;
                    let len = self.code.read_u8() as usize;
                    let n_data_words = (len + 3) / 4;
                    let total = 2 + n_data_words;
                    let addr = self.alloc(total)?;
                    self.arena.heap_write(addr, 0, Value::gc_header(total as u8));
                    self.arena.heap_write(addr, 1, Value::bytes_header(len));
                    for i in 0..n_data_words {
                        let base = i * 4;
                        let mut word: u32 = 0;
                        for j in 0..4 {
                            if base + j < len {
                                word |= (self.code.read_u8() as u32) << (j * 8);
                            }
                        }
                        self.arena.heap_write(addr, 2 + i, Value::from_u32(word));
                    }
                    self.regs[rd] = Value::bytes(addr);
                }

                opcode::BYTES_LEN => {
                    let rd = self.code.read_u8() as usize;
                    let rs = self.code.read_u8();
                    let val = self.read_reg(rs);
                    let len = self.arena.heap_read(val.bytes_addr(), 1).bytes_hdr_len();
                    self.regs[rd] = Value::int(len as i32);
                }

                opcode::BYTES_GET => {
                    let rd = self.code.read_u8() as usize;
                    let rs = self.code.read_u8();
                    let ri = self.code.read_u8();
                    let val = self.read_reg(rs);
                    let idx = self.read_reg(ri).int_value() as usize;
                    let word_idx = idx / 4;
                    let byte_off = idx % 4;
                    let word = self.arena.heap_read(val.bytes_addr(), 2 + word_idx).to_u32();
                    self.regs[rd] = Value::int(((word >> (byte_off * 8)) & 0xFF) as i32);
                }

                opcode::BYTES_CONCAT => {
                    let rd = self.code.read_u8() as usize;
                    let ra = self.code.read_u8();
                    let rb = self.code.read_u8();
                    let a = self.read_reg(ra);
                    let b = self.read_reg(rb);
                    let a_addr = a.bytes_addr();
                    let b_addr = b.bytes_addr();
                    let a_len = self.arena.heap_read(a_addr, 1).bytes_hdr_len();
                    let b_len = self.arena.heap_read(b_addr, 1).bytes_hdr_len();
                    let new_len = a_len + b_len;
                    let n_data_words = (new_len + 3) / 4;
                    let total = 2 + n_data_words;
                    let addr = self.alloc(total)?;
                    self.arena.heap_write(addr, 0, Value::gc_header(total as u8));
                    self.arena.heap_write(addr, 1, Value::bytes_header(new_len));
                    let a_addr = self.read_reg(ra).bytes_addr();
                    let b_addr = self.read_reg(rb).bytes_addr();
                    for i in 0..n_data_words {
                        let base = i * 4;
                        let mut word: u32 = 0;
                        for j in 0..4 {
                            let pos = base + j;
                            if pos < new_len {
                                let byte = if pos < a_len {
                                    let w = self.arena.heap_read(a_addr, 2 + pos / 4).to_u32();
                                    (w >> ((pos % 4) * 8)) as u8
                                } else {
                                    let p = pos - a_len;
                                    let w = self.arena.heap_read(b_addr, 2 + p / 4).to_u32();
                                    (w >> ((p % 4) * 8)) as u8
                                };
                                word |= (byte as u32) << (j * 8);
                            }
                        }
                        self.arena.heap_write(addr, 2 + i, Value::from_u32(word));
                    }
                    self.regs[rd] = Value::bytes(addr);
                }

                opcode::BYTES_SLICE => {
                    let rd = self.code.read_u8() as usize;
                    let rs = self.code.read_u8();
                    let ri = self.code.read_u8();
                    let rn = self.code.read_u8();
                    let start = self.read_reg(ri).int_value() as usize;
                    let slice_len = self.read_reg(rn).int_value() as usize;
                    let n_data_words = (slice_len + 3) / 4;
                    let total = 2 + n_data_words;
                    let addr = self.alloc(total)?;
                    self.arena.heap_write(addr, 0, Value::gc_header(total as u8));
                    self.arena.heap_write(addr, 1, Value::bytes_header(slice_len));
                    let src_addr = self.read_reg(rs).bytes_addr();
                    for i in 0..n_data_words {
                        let base = i * 4;
                        let mut word: u32 = 0;
                        for j in 0..4 {
                            if base + j < slice_len {
                                let pos = start + base + j;
                                let w = self.arena.heap_read(src_addr, 2 + pos / 4).to_u32();
                                let byte = (w >> ((pos % 4) * 8)) as u8;
                                word |= (byte as u32) << (j * 8);
                            }
                        }
                        self.arena.heap_write(addr, 2 + i, Value::from_u32(word));
                    }
                    self.regs[rd] = Value::bytes(addr);
                }

                opcode::BYTES_EQ => {
                    let rd = self.code.read_u8() as usize;
                    let ra = self.code.read_u8();
                    let rb = self.code.read_u8();
                    let a = self.read_reg(ra);
                    let b = self.read_reg(rb);
                    let a_addr = a.bytes_addr();
                    let b_addr = b.bytes_addr();
                    let a_len = self.arena.heap_read(a_addr, 1).bytes_hdr_len();
                    let b_len = self.arena.heap_read(b_addr, 1).bytes_hdr_len();
                    let eq = if a_len != b_len {
                        false
                    } else {
                        let n_words = (a_len + 3) / 4;
                        let mut equal = true;
                        for i in 0..n_words {
                            if self.arena.heap_read(a_addr, 2 + i).to_u32()
                                != self.arena.heap_read(b_addr, 2 + i).to_u32()
                            {
                                equal = false;
                                break;
                            }
                        }
                        equal
                    };
                    let tag = if eq { 1 } else { 0 };
                    self.regs[rd] = Value::ctor(tag, HeapAddress::NULL);
                }

                opcode::FIN => {
                    let rs = self.code.read_u8();
                    return Ok(self.read_reg(rs));
                }

                _ => {
                    return Err(VmError::InvalidOpcode(op));
                }
            }
        }
    }
}
