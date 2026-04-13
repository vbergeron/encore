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
                Value::function(CodeAddress::new(0)),
                Value::from_u32(0),
                Value::from_u32(0),
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
            self.globals[i] = self.call_address(addr, Value::from_u32(0))?;
        }
        Ok(())
    }

    pub fn global(&self, idx: usize) -> Value {
        self.globals[idx]
    }

    fn alloc(&mut self, n: usize) -> Result<HeapAddress, VmError> {
        self.arena.alloc(n, &mut self.registers)
    }

    fn stack_ensure(&mut self, n: usize) -> Result<(), VmError> {
        self.arena.stack_ensure(n, &mut self.registers)
    }

    pub fn call(&mut self, global_idx: usize, arg: Value) -> Result<Value, VmError> {
        let func = self.globals[global_idx];
        let code_ptr = if func.closure_ncap() == 0 {
            CodeAddress::new(func.closure_addr().raw())
        } else {
            self.arena.heap_read(func.closure_addr(), 1).header_code_ptr()
        };
        self.registers[SELF_REF] = func;
        self.arena.stack_reset();
        self.registers[ARG] = arg;
        self.registers[CONT] = Value::from_u32(0);
        self.code.jump(code_ptr);
        self.run()
    }

    fn call_address(&mut self, entry: CodeAddress, arg: Value) -> Result<Value, VmError> {
        self.registers[SELF_REF] = Value::function(entry);
        self.arena.stack_reset();
        self.registers[ARG] = arg;
        self.registers[CONT] = Value::from_u32(0);
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
                    self.stack_ensure(1)?;
                    let idx = self.code.read_u8();
                    self.arena.stack_push(self.globals[idx as usize]);
                }

                opcode::CAPTURE => {
                    self.stack_ensure(1)?;
                    let idx = self.code.read_u8();
                    let addr = self.registers[SELF_REF].closure_addr();
                    let val = self.arena.heap_read(addr, 2 + idx as usize);
                    self.arena.stack_push(val);
                }

                opcode::LOCAL => {
                    self.stack_ensure(1)?;
                    let idx = self.code.read_u8();
                    let val = self.arena.stack_local(idx);
                    self.arena.stack_push(val);
                }

                opcode::ARG => {
                    self.stack_ensure(1)?;
                    self.arena.stack_push(self.registers[ARG]);
                }

                opcode::SELF => {
                    self.stack_ensure(1)?;
                    self.arena.stack_push(self.registers[SELF_REF]);
                }

                opcode::CONT => {
                    self.stack_ensure(1)?;
                    self.arena.stack_push(self.registers[CONT]);
                }

                opcode::CLOSURE => {
                    let code_ptr = self.code.read_address();
                    let ncap = self.code.read_u8();
                    let size = 2 + ncap as usize;
                    let addr = self.alloc(size)?;
                    self.arena.heap_write(addr, 0, Value::gc_header(size as u8));
                    self.arena.heap_write(addr, 1, Value::closure_header(code_ptr));
                    for i in 0..ncap as usize {
                        let val = self.arena.stack_pop();
                        self.arena.heap_write(addr, 2 + ncap as usize - 1 - i, val);
                    }
                    self.arena.stack_push(Value::closure(ncap, addr));
                }

                opcode::FUNCTION => {
                    self.stack_ensure(1)?;
                    let code_ptr = self.code.read_address();
                    self.arena.stack_push(Value::function(code_ptr));
                }

                opcode::PACK => {
                    let tag = self.code.read_u8();
                    let arity = self.arity_table[tag as usize] as usize;
                    if arity == 0 {
                        self.stack_ensure(1)?;
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
                    if arity > 1 {
                        self.stack_ensure(arity - 1)?;
                    }
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
                    debug_assert!(branch < n as usize);
                    let off = self.code.read_address_at(table + branch * 2);
                    self.code.jump(off);
                }

                opcode::ENCORE => {
                    let clo = self.arena.stack_pop();
                    let arg = self.arena.stack_pop();
                    let cont = self.arena.stack_pop();
                    let code_ptr = if clo.closure_ncap() == 0 {
                        CodeAddress::new(clo.closure_addr().raw())
                    } else {
                        self.arena.heap_read(clo.closure_addr(), 1).header_code_ptr()
                    };
                    self.registers[SELF_REF] = clo;
                    self.registers[ARG] = arg;
                    self.registers[CONT] = cont;
                    self.arena.stack_reset();
                    self.code.jump(code_ptr);
                }

                opcode::RETURN => {
                    let clo = self.arena.stack_pop();
                    let result = self.arena.stack_pop();
                    let code_ptr = if clo.closure_ncap() == 0 {
                        CodeAddress::new(clo.closure_addr().raw())
                    } else {
                        self.arena.heap_read(clo.closure_addr(), 1).header_code_ptr()
                    };
                    self.registers[SELF_REF] = clo;
                    self.registers[ARG] = result;
                    self.arena.stack_reset();
                    self.code.jump(code_ptr);
                }

                opcode::INT => {
                    self.stack_ensure(1)?;
                    let b0 = self.code.read_u8() as u32;
                    let b1 = self.code.read_u8() as u32;
                    let b2 = self.code.read_u8() as u32;
                    let raw = b0 | (b1 << 8) | (b2 << 16);
                    let n = ((raw as i32) << 8) >> 8;
                    self.arena.stack_push(Value::int(n));
                }

                opcode::INT_0 => {
                    self.stack_ensure(1)?;
                    self.arena.stack_push(Value::int(0));
                }

                opcode::INT_1 => {
                    self.stack_ensure(1)?;
                    self.arena.stack_push(Value::int(1));
                }

                opcode::INT_2 => {
                    self.stack_ensure(1)?;
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
