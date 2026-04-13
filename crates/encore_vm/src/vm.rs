use crate::arena::Arena;
use crate::code::Code;
use crate::error::VmError;
use crate::opcode;
use crate::program::Program;
#[cfg(feature = "stats")]
use crate::stats::VmStats;
use crate::value::{CodeAddress, HeapAddress, Value};

const SELF_REF: usize = 0;
const ARG: usize = 1;
const CONT: usize = 2;

pub type ExternFn = fn(Value) -> Value;
const MAX_EXTERN: usize = 32;

pub struct Vm<'a> {
    code: Code<'a>,
    arity_table: &'a [u8],
    globals: [Value; 64],
    n_globals: u8,
    extern_fns: [Option<ExternFn>; MAX_EXTERN],
    arena: Arena<'a>,
    registers: [Value; 3],
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
            registers: [
                Value::function(0, CodeAddress::new(0)),
                Value::int(0),
                Self::RETURN_CONT,
            ],
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
            let sd = prog.global_sd(i);
            self.globals[i] = self.call_address(addr, sd, Value::from_u32(0))?;
        }
        Ok(())
    }

    pub fn global(&self, idx: usize) -> Value {
        self.globals[idx]
    }

    pub fn ctor_field(&self, val: Value, idx: usize) -> Value {
        self.arena.heap_read(val.ctor_addr(), 1 + idx)
    }

    fn alloc(&mut self, n: usize) -> Result<HeapAddress, VmError> {
        self.arena.alloc(n, &mut self.registers, &mut self.globals[..self.n_globals as usize])
    }

    fn stack_reserve(&mut self, sd: u8) -> Result<(), VmError> {
        self.arena.stack_reserve(sd as usize, &mut self.registers, &mut self.globals[..self.n_globals as usize])
    }

    fn resolve_code_ptr(&self, func: Value) -> CodeAddress {
        if func.is_function() {
            func.code_ptr()
        } else {
            self.arena.heap_read(func.closure_addr(), 1).header_code_ptr()
        }
    }

    const RETURN_CONT: Value = Value::function_const(0, 0);

    pub fn call(&mut self, global_idx: usize, arg: Value) -> Result<Value, VmError> {
        let func = self.globals[global_idx];
        let code_ptr = self.resolve_code_ptr(func);
        self.registers[SELF_REF] = func;
        self.arena.stack_reset();
        self.stack_reserve(func.stack_delta())?;
        self.registers[ARG] = arg;
        self.registers[CONT] = Self::RETURN_CONT;
        self.code.jump(code_ptr);
        self.run()
    }

    pub fn call_value(&mut self, func: Value, arg: Value) -> Result<Value, VmError> {
        let code_ptr = self.resolve_code_ptr(func);
        self.registers[SELF_REF] = func;
        self.arena.stack_reset();
        self.stack_reserve(func.stack_delta())?;
        self.registers[ARG] = arg;
        self.registers[CONT] = Self::RETURN_CONT;
        self.code.jump(code_ptr);
        self.run()
    }

    fn call_address(&mut self, entry: CodeAddress, sd: u8, arg: Value) -> Result<Value, VmError> {
        self.registers[SELF_REF] = Value::function(sd, entry);
        self.arena.stack_reset();
        self.stack_reserve(sd)?;
        self.registers[ARG] = arg;
        self.registers[CONT] = Self::RETURN_CONT;
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
                opcode::GLOBAL => {
                    let idx = self.code.read_u8();
                    self.arena.stack_push(self.globals[idx as usize]);
                }

                opcode::CAPTURE => {
                    let idx = self.code.read_u8();
                    let addr = self.registers[SELF_REF].closure_addr();
                    let val = self.arena.heap_read(addr, 2 + idx as usize);
                    self.arena.stack_push(val);
                }

                opcode::LOCAL => {
                    let idx = self.code.read_u8();
                    let val = self.arena.stack_local(idx);
                    self.arena.stack_push(val);
                }

                opcode::ARG => {
                    self.arena.stack_push(self.registers[ARG]);
                }

                opcode::SELF => {
                    self.arena.stack_push(self.registers[SELF_REF]);
                }

                opcode::CONT => {
                    self.arena.stack_push(self.registers[CONT]);
                }

                opcode::CLOSURE => {
                    let code_ptr = self.code.read_address();
                    let ncap = self.code.read_u8();
                    let sd = self.code.read_u8();
                    let size = 2 + ncap as usize;
                    let addr = self.alloc(size)?;
                    self.arena.heap_write(addr, 0, Value::gc_header(size as u8));
                    self.arena.heap_write(addr, 1, Value::closure_header(ncap, code_ptr));
                    for i in 0..ncap as usize {
                        let val = self.arena.stack_pop();
                        self.arena.heap_write(addr, 2 + ncap as usize - 1 - i, val);
                    }
                    self.arena.stack_push(Value::closure(sd, addr));
                }

                opcode::FUNCTION => {
                    let code_ptr = self.code.read_address();
                    let sd = self.code.read_u8();
                    self.arena.stack_push(Value::function(sd, code_ptr));
                }

                opcode::PACK => {
                    let tag = self.code.read_u8();
                    let arity = self.arity_table[tag as usize] as usize;
                    if arity == 0 {
                        self.arena.stack_push(Value::ctor(tag, HeapAddress::NULL));
                    } else {
                        let size = 1 + arity;
                        let addr = self.alloc(size)?;
                        self.arena.heap_write(addr, 0, Value::gc_header(size as u8));
                        for i in 0..arity {
                            let val = self.arena.stack_pop();
                            self.arena.heap_write(addr, 1 + arity - 1 - i, val);
                        }
                        self.arena.stack_push(Value::ctor(tag, addr));
                    }
                }

                opcode::FIELD => {
                    let i = self.code.read_u8();
                    let ctor = self.arena.stack_pop();
                    let val = self.arena.heap_read(ctor.ctor_addr(), 1 + i as usize);
                    self.arena.stack_push(val);
                }

                opcode::UNPACK => {
                    let tag = self.code.read_u8();
                    let arity = self.arity_table[tag as usize] as usize;
                    let ctor = self.arena.stack_pop();
                    let addr = ctor.ctor_addr();
                    for i in 0..arity {
                        self.arena.stack_push(self.arena.heap_read(addr, 1 + i));
                    }
                }

                opcode::MATCH => {
                    let base = self.code.read_u8();
                    let n = self.code.read_u8();
                    let table = self.code.pc();
                    self.code.skip(n as usize * 2);
                    let ctor = self.arena.stack_pop();
                    let branch = ctor.ctor_tag().wrapping_sub(base) as usize;
                    if branch >= n as usize {
                        return Err(VmError::MatchFail);
                    }
                    let off = self.code.read_address_at(table + branch * 2);
                    self.code.jump(off);
                }

                opcode::ENCORE => {
                    let clo = self.arena.stack_pop();
                    let arg = self.arena.stack_pop();
                    let cont = self.arena.stack_pop();
                    let code_ptr = self.resolve_code_ptr(clo);
                    self.registers[SELF_REF] = clo;
                    self.registers[ARG] = arg;
                    self.registers[CONT] = cont;
                    self.arena.stack_reset();
                    self.stack_reserve(clo.stack_delta())?;
                    self.code.jump(code_ptr);
                }

                opcode::NULLADDR => {
                    self.arena.stack_push(Value::from_u32(0xFFFF));
                }

                opcode::INT => {
                    let b0 = self.code.read_u8() as u32;
                    let b1 = self.code.read_u8() as u32;
                    let b2 = self.code.read_u8() as u32;
                    let raw = b0 | (b1 << 8) | (b2 << 16);
                    let n = ((raw as i32) << 8) >> 8;
                    self.arena.stack_push(Value::int(n));
                }

                opcode::INT_0 => {
                    self.arena.stack_push(Value::int(0));
                }

                opcode::INT_1 => {
                    self.arena.stack_push(Value::int(1));
                }

                opcode::INT_2 => {
                    self.arena.stack_push(Value::int(2));
                }

                opcode::INT_ADD => {
                    let b = self.arena.stack_pop().int_value();
                    let a = self.arena.stack_pop().int_value();
                    self.arena.stack_push(Value::int(a.wrapping_add(b)));
                }

                opcode::INT_SUB => {
                    let b = self.arena.stack_pop().int_value();
                    let a = self.arena.stack_pop().int_value();
                    self.arena.stack_push(Value::int(a.wrapping_sub(b)));
                }

                opcode::INT_MUL => {
                    let b = self.arena.stack_pop().int_value();
                    let a = self.arena.stack_pop().int_value();
                    self.arena.stack_push(Value::int(a.wrapping_mul(b)));
                }

                opcode::INT_EQ => {
                    let b = self.arena.stack_pop().int_value();
                    let a = self.arena.stack_pop().int_value();
                    let tag = if a == b { 1 } else { 0 };
                    self.arena.stack_push(Value::ctor(tag, HeapAddress::NULL));
                }

                opcode::INT_LT => {
                    let b = self.arena.stack_pop().int_value();
                    let a = self.arena.stack_pop().int_value();
                    let tag = if a < b { 1 } else { 0 };
                    self.arena.stack_push(Value::ctor(tag, HeapAddress::NULL));
                }

                opcode::EXTERN => {
                    let idx = self.code.read_u16();
                    let arg = self.arena.stack_pop();
                    let f = self.extern_fns[idx as usize]
                        .ok_or(VmError::NotRegistered(idx))?;
                    let result = f(arg);
                    self.arena.stack_push(result);
                }

                opcode::FIN => {
                    return Ok(self.arena.stack_peek());
                }

                _ => {
                    return Err(VmError::InvalidOpcode(op));
                }
            }
        }
    }
}
