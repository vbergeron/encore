use crate::arena::Arena;
use crate::code::Code;
use crate::error::VmError;
use crate::opcode;
#[cfg(feature = "stats")]
use crate::stats::VmStats;
use crate::value::{CodeAddress, HeapAddress, Value};

pub struct Vm<'a> {
    code: Code<'a>,
    arity_table: &'a [u8],
    globals: &'a [Value],
    arena: Arena<'a>,
    self_ref: Value,
    arg: Value,
    cont: Value,
    #[cfg(feature = "stats")]
    stats: VmStats,
}

impl<'a> Vm<'a> {
    pub fn new(
        code: &'a [u8],
        arity_table: &'a [u8],
        globals: &'a [Value],
        mem: &'a mut [Value],
    ) -> Self {
        Self {
            code: Code::new(code),
            arity_table,
            globals,
            arena: Arena::new(mem),
            self_ref: Value::function(CodeAddress::new(0)),
            arg: Value::from_u32(0),
            cont: Value::from_u32(0),
            #[cfg(feature = "stats")]
            stats: VmStats::default(),
        }
    }

    fn alloc(&mut self, n: usize) -> Result<HeapAddress, VmError> {
        self.arena.alloc(n, &mut self.self_ref, &mut self.arg, &mut self.cont)
    }

    pub fn call(&mut self, entry: CodeAddress, arg: Value) -> Result<Value, VmError> {
        self.self_ref = Value::function(entry);
        self.arena.stack_reset();
        self.arg = arg;
        self.cont = Value::from_u32(0);
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

    pub fn run(&mut self) -> Result<Value, VmError> {
        loop {
            #[cfg(feature = "stats")]
            { self.stats.op_count += 1; }
            let op = self.code.read_u8();
            match op {
                opcode::GLOBAL => {
                    self.arena.stack_ensure(1)?;
                    let idx = self.code.read_u8();
                    self.arena.stack_push(self.globals[idx as usize]);
                }

                opcode::CAPTURE => {
                    self.arena.stack_ensure(1)?;
                    let idx = self.code.read_u8();
                    let addr = self.self_ref.closure_addr();
                    let val = self.arena.heap_read(addr, 2 + idx as usize);
                    self.arena.stack_push(val);
                }

                opcode::LOCAL => {
                    self.arena.stack_ensure(1)?;
                    let idx = self.code.read_u8();
                    let val = self.arena.stack_local(idx);
                    self.arena.stack_push(val);
                }

                opcode::ARG => {
                    self.arena.stack_ensure(1)?;
                    self.arena.stack_push(self.arg);
                }

                opcode::SELF => {
                    self.arena.stack_ensure(1)?;
                    self.arena.stack_push(self.self_ref);
                }

                opcode::CONT => {
                    self.arena.stack_ensure(1)?;
                    self.arena.stack_push(self.cont);
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
                    self.arena.stack_ensure(1)?;
                    let code_ptr = self.code.read_address();
                    self.arena.stack_push(Value::function(code_ptr));
                }

                opcode::PACK => {
                    let tag = self.code.read_u8();
                    let arity = self.arity_table[tag as usize] as usize;
                    if arity == 0 {
                        self.arena.stack_ensure(1)?;
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
                    self.self_ref = clo;
                    self.arg = arg;
                    self.cont = cont;
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
                    self.self_ref = clo;
                    self.arg = result;
                    self.arena.stack_reset();
                    self.code.jump(code_ptr);
                }

                opcode::INT => {
                    self.arena.stack_ensure(1)?;
                    let b0 = self.code.read_u8() as u32;
                    let b1 = self.code.read_u8() as u32;
                    let b2 = self.code.read_u8() as u32;
                    let raw = b0 | (b1 << 8) | (b2 << 16);
                    let n = ((raw as i32) << 8) >> 8;
                    self.arena.stack_push(Value::int(n));
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
