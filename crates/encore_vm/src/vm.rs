use crate::arena::Arena;
use crate::code::Code;
use crate::error::{ExternError, VmError};
use crate::ffi::{EncodeArgs, ValueDecode, VmCallable};
use crate::gc;
use crate::opcode;
use crate::program::Program;
use crate::registers::Registers;
#[cfg(feature = "stats")]
use crate::stats::VmStats;
use crate::value::{CodeAddress, GlobalAddress, HeapAddress, Reg, Value};

const SELF: Reg = Reg::new(0);
const CONT: Reg = Reg::new(1);
const A1: Reg = Reg::new(2);

pub type ExternFn = fn(&mut Vm, Value) -> Result<Value, ExternError>;
const MAX_EXTERN: usize = 32;

fn unregistered(_: &mut Vm, _: Value) -> Result<Value, ExternError> {
    Err(ExternError::Unregistered)
}

pub struct Vm<'a> {
    code: Code<'a>,
    arity_table: &'a [u8],
    globals: [Value; 64],
    n_globals: u8,
    extern_fns: [ExternFn; MAX_EXTERN],
    arena: Arena<'a>,
    registers: Registers,
    executing_extern: bool,
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
            extern_fns: [unregistered; MAX_EXTERN],
            arena: Arena::new(mem),
            registers: Registers::new(),
            executing_extern: false,
            #[cfg(feature = "stats")]
            stats: VmStats::default(),
        }
    }

    pub fn register_extern(&mut self, slot: u16, f: ExternFn) {
        self.extern_fns[slot as usize] = f;
    }

    pub fn load(&mut self, prog: &Program<'a>) -> Result<(), VmError> {
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
        self.arena[val.ctor_addr() + 1 + idx]
    }

    pub fn alloc_ctor(&mut self, tag: u8, fields: &[Value]) -> Result<Value, VmError> {
        if fields.is_empty() {
            return Ok(Value::ctor(tag, HeapAddress::NULL));
        }
        let size = 1 + fields.len();
        let addr = self.alloc(size)?;
        self.arena[addr] = Value::gc_header(size as u8);
        for (i, &f) in fields.iter().enumerate() {
            self.arena[addr + 1 + i] = f;
        }
        Ok(Value::ctor(tag, addr))
    }

    pub fn alloc_bytes(&mut self, data: &[u8]) -> Result<Value, VmError> {
        let n_data_words = (data.len() + 3) / 4;
        let total = 2 + n_data_words;
        let addr = self.alloc(total)?;
        self.arena[addr + 0] = Value::gc_header(total as u8);
        self.arena[addr + 1] = Value::bytes_header(data.len());
        for i in 0..n_data_words {
            let base = i * 4;
            let mut word: u32 = 0;
            for j in 0..4 {
                if base + j < data.len() {
                    word |= (data[base + j] as u32) << (j * 8);
                }
            }
            self.arena[addr + 2 + i] = Value::from_u32(word);
        }
        Ok(Value::bytes(addr))
    }

    pub fn bytes_len(&self, val: Value) -> usize {
        self.arena[val.bytes_addr() + 1].bytes_hdr_len()
    }

    pub fn bytes_read(&self, val: Value, idx: usize) -> u8 {
        let word_idx = idx / 4;
        let byte_off = idx % 4;
        let word = self.arena[val.bytes_addr() + 2 + word_idx].to_u32();
        (word >> (byte_off * 8)) as u8
    }

    pub fn bytes_slice<'b>(&self, val: Value, buf: &'b mut [u8]) -> &'b [u8] {
        let len = self.bytes_len(val);
        let n = if len < buf.len() { len } else { buf.len() };
        for i in 0..n {
            buf[i] = self.bytes_read(val, i);
        }
        &buf[..n]
    }

    #[inline(always)]
    fn alloc(&mut self, n: usize) -> Result<HeapAddress, VmError> {
        self.arena.try_alloc(n).or_else(|_| self.alloc_slow(n))
    }

    #[cold]
    #[inline(never)]
    fn alloc_slow(&mut self, n: usize) -> Result<HeapAddress, VmError> {
        if self.executing_extern {
            return Err(VmError::HeapOverflow);
        }
        let roots = self.registers.as_mut_slice();
        let globals = &mut self.globals[..self.n_globals as usize];
        gc::collect(&mut self.arena, roots, globals);
        self.arena.try_alloc(n)
    }

    fn resolve_code_ptr(&self, func: Value) -> CodeAddress {
        if func.is_function() {
            func.code_ptr()
        } else {
            self.arena[func.closure_addr() + 1].header_code_ptr()
        }
    }

    const RETURN_CONT: Value = Value::function_const(0);

    fn call_raw(&mut self, func: Value, args: &[Value]) -> Result<Value, VmError> {
        let code_ptr = self.resolve_code_ptr(func);
        self.registers[SELF] = func;
        self.registers[CONT] = Self::RETURN_CONT;
        for (i, arg) in args.iter().enumerate() {
            self.registers[Reg::new(2 + i as u8)] = *arg;
        }
        self.code.jump(code_ptr);
        self.run()
    }

    pub fn call_global_raw(&mut self, global_idx: GlobalAddress, args: &[Value]) -> Result<Value, VmError> {
        let func = self.globals[global_idx.raw() as usize];
        self.call_raw(func, args)
    }

    pub fn call_global<Args, O>(&mut self, global_idx: GlobalAddress, args: Args) -> Result<O, ExternError>
    where
        Args: EncodeArgs,
        O: ValueDecode,
    {
        let encoded = args.encode_args(self)?;
        let raw = self.call_global_raw(global_idx, encoded.as_ref())?;
        O::decode(self, raw).map_err(ExternError::from)
    }

    pub fn call_closure_raw(&mut self, callable: VmCallable, args: &[Value]) -> Result<Value, VmError> {
        self.call_raw(callable.raw(), args)
    }

    pub fn call_closure<Args, O>(&mut self, callable: VmCallable, args: Args) -> Result<O, ExternError>
    where
        Args: EncodeArgs,
        O: ValueDecode,
    {
        let encoded = args.encode_args(self)?;
        let raw = self.call_closure_raw(callable, encoded.as_ref())?;
        O::decode(self, raw).map_err(ExternError::from)
    }

    fn call_address(&mut self, entry: CodeAddress, arg: Value) -> Result<Value, VmError> {
        self.registers[SELF] = Value::function(entry);
        self.registers[CONT] = Self::RETURN_CONT;
        self.registers[A1] = arg;
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
            let pc = self.code.pc() as u16;
            let op = self.code.read_u8();
            match op {
                opcode::MOV => {
                    let rd = self.code.read_reg();
                    let rs = self.code.read_reg();
                    self.registers[rd] = self.registers[rs];
                }

                opcode::CAPTURE => {
                    let rd = self.code.read_reg();
                    let idx = self.code.read_u8() as usize;
                    let addr = self.registers[SELF].closure_addr();
                    self.registers[rd] = self.arena[addr + 2 + idx];
                }

                opcode::GLOBAL => {
                    let rd = self.code.read_reg();
                    let idx = self.code.read_u8() as usize;
                    self.registers[rd] = self.globals[idx];
                }

                opcode::CLOSURE => {
                    let rd = self.code.read_reg();
                    let code_ptr = self.code.read_address();
                    let ncap = self.code.read_u8();
                    let size = 2 + ncap as usize;
                    let addr = self.alloc(size)?;
                    self.arena[addr + 0] = Value::gc_header(size as u8);
                    self.arena[addr + 1] = Value::closure_header(ncap, code_ptr);
                    for i in 0..ncap as usize {
                        let cap = self.code.read_reg();
                        self.arena[addr + 2 + i] = self.registers[cap];
                    }
                    self.registers[rd] = Value::closure(addr);
                }

                opcode::FUNCTION => {
                    let rd = self.code.read_reg();
                    let code_ptr = self.code.read_address();
                    self.registers[rd] = Value::function(code_ptr);
                }

                opcode::PACK => {
                    let rd = self.code.read_reg();
                    let tag = self.code.read_u8();
                    let arity = self.arity_table[tag as usize] as usize;
                    if arity == 0 {
                        self.registers[rd] = Value::ctor(tag, HeapAddress::NULL);
                    } else {
                        let size = 1 + arity;
                        let addr = self.alloc(size)?;
                        self.arena[addr] = Value::gc_header(size as u8);
                        for i in 0..arity {
                            let field = self.code.read_reg();
                            self.arena[addr + 1 + i] = self.registers[field];
                        }
                        self.registers[rd] = Value::ctor(tag, addr);
                    }
                }

                opcode::FIELD => {
                    let rd = self.code.read_reg();
                    let rs = self.code.read_reg();
                    let idx = self.code.read_u8() as usize;
                    let ctor = self.registers[rs];
                    self.registers[rd] = self.arena[ctor.ctor_addr() + 1 + idx];
                }

                opcode::UNPACK => {
                    let rd = self.code.read_reg();
                    let tag = self.code.read_u8();
                    let rs = self.code.read_reg();
                    let arity = self.arity_table[tag as usize] as usize;
                    let ctor = self.registers[rs];
                    let addr = ctor.ctor_addr();
                    for i in 0..arity {
                        self.registers[rd + i] = self.arena[addr + 1 + i];
                    }
                }

                opcode::MATCH => {
                    let rs = self.code.read_reg();
                    let base = self.code.read_u8();
                    let n = self.code.read_u8();
                    let table = self.code.pc();
                    self.code.skip(n as usize * 2);
                    let ctor = self.registers[rs];
                    let tag = ctor.ctor_tag();
                    let branch = tag.wrapping_sub(base) as usize;
                    if branch >= n as usize {
                        return Err(VmError::MatchFail { tag, pc });
                    }
                    let off = self.code.read_address_at(table + branch * 2);
                    self.code.jump(off);
                }

                opcode::BRANCH => {
                    let rs = self.code.read_reg();
                    let base = self.code.read_u8();
                    let addr0 = self.code.read_address();
                    let addr1 = self.code.read_address();
                    let tag = self.registers[rs].ctor_tag();
                    self.code.jump(if tag == base { addr0 } else { addr1 });
                }

                opcode::ENCORE => {
                    let rf = self.code.read_reg();
                    let rk = self.code.read_reg();
                    let fun = self.registers[rf];
                    let cont = self.registers[rk];
                    let code_ptr = self.resolve_code_ptr(fun);
                    self.registers[SELF] = fun;
                    self.registers[CONT] = cont;
                    self.code.jump(code_ptr);
                }

                opcode::INT => {
                    let rd = self.code.read_reg();
                    let raw = self.code.read_u24();
                    let n = ((raw as i32) << 8) >> 8;
                    self.registers[rd] = Value::int(n);
                }

                opcode::INT_0 => {
                    let rd = self.code.read_reg();
                    self.registers[rd] = Value::int(0);
                }

                opcode::INT_1 => {
                    let rd = self.code.read_reg();
                    self.registers[rd] = Value::int(1);
                }

                opcode::INT_2 => {
                    let rd = self.code.read_reg();
                    self.registers[rd] = Value::int(2);
                }

                opcode::INT_ADD => {
                    let rd = self.code.read_reg();
                    let ra = self.code.read_reg();
                    let rb = self.code.read_reg();
                    let a = self.registers[ra].int_value()?;
                    let b = self.registers[rb].int_value()?;
                    self.registers[rd] = Value::int(a.wrapping_add(b));
                }

                opcode::INT_SUB => {
                    let rd = self.code.read_reg();
                    let ra = self.code.read_reg();
                    let rb = self.code.read_reg();
                    let a = self.registers[ra].int_value()?;
                    let b = self.registers[rb].int_value()?;
                    self.registers[rd] = Value::int(a.wrapping_sub(b));
                }

                opcode::INT_MUL => {
                    let rd = self.code.read_reg();
                    let ra = self.code.read_reg();
                    let rb = self.code.read_reg();
                    let a = self.registers[ra].int_value()?;
                    let b = self.registers[rb].int_value()?;
                    self.registers[rd] = Value::int(a.wrapping_mul(b));
                }

                opcode::INT_EQ => {
                    let rd = self.code.read_reg();
                    let ra = self.code.read_reg();
                    let rb = self.code.read_reg();
                    let a = self.registers[ra].int_value()?;
                    let b = self.registers[rb].int_value()?;
                    let tag = if a == b { 1 } else { 0 };
                    self.registers[rd] = Value::ctor(tag, HeapAddress::NULL);
                }

                opcode::INT_LT => {
                    let rd = self.code.read_reg();
                    let ra = self.code.read_reg();
                    let rb = self.code.read_reg();
                    let a = self.registers[ra].int_value()?;
                    let b = self.registers[rb].int_value()?;
                    let tag = if a < b { 1 } else { 0 };
                    self.registers[rd] = Value::ctor(tag, HeapAddress::NULL);
                }

                opcode::INT_BYTE => {
                    let rd = self.code.read_reg();
                    let rs = self.code.read_reg();
                    let n = self.registers[rs].int_value()?;
                    if n < 0 || n > 255 {
                        return Err(VmError::ByteRange { value: n, pc });
                    }
                    let addr = self.alloc(3)?;
                    self.arena[addr + 0] = Value::gc_header(3);
                    self.arena[addr + 1] = Value::bytes_header(1);
                    self.arena[addr + 2] = Value::from_u32(n as u32);
                    self.registers[rd] = Value::bytes(addr);
                }

                opcode::EXTERN => {
                    let rd = self.code.read_reg();
                    let ra = self.code.read_reg();
                    let idx = self.code.read_u16();
                    let arg = self.registers[ra];
                    let f = self.extern_fns[idx as usize];
                    self.executing_extern = true;
                    let result = f(self, arg);
                    self.executing_extern = false;
                    let result = result.map_err(|error| VmError::Extern { error, slot: idx, pc })?;
                    self.registers[rd] = result;
                }

                opcode::BYTES => {
                    let rd = self.code.read_reg();
                    let len = self.code.read_u8() as usize;
                    let n_data_words = (len + 3) / 4;
                    let total = 2 + n_data_words;
                    let addr = self.alloc(total)?;
                    self.arena[addr + 0] = Value::gc_header(total as u8);
                    self.arena[addr + 1] = Value::bytes_header(len);
                    for i in 0..n_data_words {
                        let base = i * 4;
                        let mut word: u32 = 0;
                        for j in 0..4 {
                            if base + j < len {
                                word |= (self.code.read_u8() as u32) << (j * 8);
                            }
                        }
                        self.arena[addr + 2 + i] = Value::from_u32(word);
                    }
                    self.registers[rd] = Value::bytes(addr);
                }

                opcode::BYTES_LEN => {
                    let rd = self.code.read_reg();
                    let rs = self.code.read_reg();
                    let val = self.registers[rs];
                    let len = self.arena[val.bytes_addr() + 1].bytes_hdr_len();
                    self.registers[rd] = Value::int(len as i32);
                }

                opcode::BYTES_GET => {
                    let rd = self.code.read_reg();
                    let rs = self.code.read_reg();
                    let ri = self.code.read_reg();
                    let val = self.registers[rs];
                    let idx = self.registers[ri].int_value()? as usize;
                    let word_idx = idx / 4;
                    let byte_off = idx % 4;
                    let word = self.arena[val.bytes_addr() + 2 + word_idx].to_u32();
                    self.registers[rd] = Value::int(((word >> (byte_off * 8)) & 0xFF) as i32);
                }

                opcode::BYTES_CONCAT => {
                    let rd = self.code.read_reg();
                    let ra = self.code.read_reg();
                    let rb = self.code.read_reg();
                    let a = self.registers[ra];
                    let b = self.registers[rb];
                    let a_addr = a.bytes_addr();
                    let b_addr = b.bytes_addr();
                    let a_len = self.arena[a_addr + 1].bytes_hdr_len();
                    let b_len = self.arena[b_addr + 1].bytes_hdr_len();
                    let new_len = a_len + b_len;
                    let n_data_words = (new_len + 3) / 4;
                    let total = 2 + n_data_words;
                    let addr = self.alloc(total)?;
                    self.arena[addr + 0] = Value::gc_header(total as u8);
                    self.arena[addr + 1] = Value::bytes_header(new_len);
                    let a_addr = self.registers[ra].bytes_addr();
                    let b_addr = self.registers[rb].bytes_addr();
                    for i in 0..n_data_words {
                        let base = i * 4;
                        let mut word: u32 = 0;
                        for j in 0..4 {
                            let pos = base + j;
                            if pos < new_len {
                                let byte = if pos < a_len {
                                    let w = self.arena[a_addr + 2 + pos / 4].to_u32();
                                    (w >> ((pos % 4) * 8)) as u8
                                } else {
                                    let p = pos - a_len;
                                    let w = self.arena[b_addr + 2 + p / 4].to_u32();
                                    (w >> ((p % 4) * 8)) as u8
                                };
                                word |= (byte as u32) << (j * 8);
                            }
                        }
                        self.arena[addr + 2 + i] = Value::from_u32(word);
                    }
                    self.registers[rd] = Value::bytes(addr);
                }

                opcode::BYTES_SLICE => {
                    let rd = self.code.read_reg();
                    let rs = self.code.read_reg();
                    let ri = self.code.read_reg();
                    let rn = self.code.read_reg();
                    let start = self.registers[ri].int_value()? as usize;
                    let slice_len = self.registers[rn].int_value()? as usize;
                    let n_data_words = (slice_len + 3) / 4;
                    let total = 2 + n_data_words;
                    let addr = self.alloc(total)?;
                    self.arena[addr + 0] = Value::gc_header(total as u8);
                    self.arena[addr + 1] = Value::bytes_header(slice_len);
                    let src_addr = self.registers[rs].bytes_addr();
                    for i in 0..n_data_words {
                        let base = i * 4;
                        let mut word: u32 = 0;
                        for j in 0..4 {
                            if base + j < slice_len {
                                let pos = start + base + j;
                                let w = self.arena[src_addr + 2 + pos / 4].to_u32();
                                let byte = (w >> ((pos % 4) * 8)) as u8;
                                word |= (byte as u32) << (j * 8);
                            }
                        }
                        self.arena[addr + 2 + i] = Value::from_u32(word);
                    }
                    self.registers[rd] = Value::bytes(addr);
                }

                opcode::BYTES_EQ => {
                    let rd = self.code.read_reg();
                    let ra = self.code.read_reg();
                    let rb = self.code.read_reg();
                    let a = self.registers[ra];
                    let b = self.registers[rb];
                    let a_addr = a.bytes_addr();
                    let b_addr = b.bytes_addr();
                    let a_len = self.arena[a_addr + 1].bytes_hdr_len();
                    let b_len = self.arena[b_addr + 1].bytes_hdr_len();
                    let eq = if a_len != b_len {
                        false
                    } else {
                        let n_words = (a_len + 3) / 4;
                        let mut equal = true;
                        for i in 0..n_words {
                            if self.arena[a_addr + 2 + i].to_u32()
                                != self.arena[b_addr + 2 + i].to_u32()
                            {
                                equal = false;
                                break;
                            }
                        }
                        equal
                    };
                    let tag = if eq { 1 } else { 0 };
                    self.registers[rd] = Value::ctor(tag, HeapAddress::NULL);
                }

                opcode::FIN => {
                    let rs = self.code.read_reg();
                    return Ok(self.registers[rs]);
                }

                _ => {
                    return Err(VmError::InvalidOpcode { opcode: op, pc });
                }
            }
        }
    }
}
