use crate::arena::Arena;
use crate::code::Code;
use crate::error::VmError;
use crate::opcode;
use crate::value::{CodeAddress, HeapAddress, Value};

pub struct Vm<'a> {
    code: Code<'a>,
    arity_table: &'a [u8],
    globals: &'a [Value],
    arena: Arena<'a>,
    self_ref: Value,
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
            self_ref: Value::from_u32(0),
        }
    }

    pub fn call(&mut self, entry: CodeAddress, arg: Value) -> Result<Value, VmError> {
        let addr = self.arena.alloc(2)?;
        self.arena.heap_write(addr, 0, Value::gc_header(2));
        self.arena.heap_write(addr, 1, Value::closure_header(entry));
        self.self_ref = Value::closure(0, addr);
        self.arena.stack_reset();
        self.arena.stack_push(arg);
        self.code.jump(entry);
        self.run()
    }

    pub fn run(&mut self) -> Result<Value, VmError> {
        loop {
            let op = self.code.read_u8();
            match op {
                opcode::LOAD => {
                    self.arena.stack_ensure(1)?;
                    let idx = self.code.read_u8();
                    let val = if idx & 0x80 != 0 {
                        self.globals[(idx & 0x7F) as usize]
                    } else {
                        let addr = self.self_ref.closure_addr();
                        self.arena.heap_read(addr, 2 + idx as usize)
                    };
                    self.arena.stack_push(val);
                }

                opcode::FIX => {
                    self.arena.stack_ensure(1)?;
                    self.arena.stack_push(self.self_ref);
                }

                opcode::CLOSURE => {
                    let code_ptr = self.code.read_address();
                    let ncap = self.code.read_u8();
                    let size = 2 + ncap as usize;
                    let addr = self.arena.alloc(size)?;
                    self.arena.heap_write(addr, 0, Value::gc_header(size as u8));
                    self.arena.heap_write(addr, 1, Value::closure_header(code_ptr));
                    for i in 0..ncap as usize {
                        let val = self.arena.stack_pop();
                        self.arena.heap_write(addr, 2 + ncap as usize - 1 - i, val);
                    }
                    self.arena.stack_push(Value::closure(ncap, addr));
                }

                opcode::PACK => {
                    let tag = self.code.read_u8();
                    let arity = self.arity_table[tag as usize] as usize;
                    if arity == 0 {
                        self.arena.stack_ensure(1)?;
                        self.arena.stack_push(Value::ctor(tag, HeapAddress::NULL));
                    } else {
                        let size = 1 + arity;
                        let addr = self.arena.alloc(size)?;
                        self.arena.heap_write(addr, 0, Value::gc_header(size as u8));
                        for i in 0..arity {
                            let val = self.arena.stack_pop();
                            self.arena.heap_write(addr, 1 + arity - 1 - i, val);
                        }
                        self.arena.stack_push(Value::ctor(tag, addr));
                    }
                }

                opcode::FIELD => {
                    self.arena.stack_ensure(1)?;
                    let i = self.code.read_u8();
                    let ctor = self.arena.stack_peek();
                    let val = self.arena.heap_read(ctor.ctor_addr(), 1 + i as usize);
                    self.arena.stack_push(val);
                }

                opcode::MATCH => {
                    let base = self.code.read_u8();
                    let n = self.code.read_u8();
                    let table = self.code.pc();
                    self.code.skip(n as usize * 2);
                    let ctor = self.arena.stack_peek();
                    let branch = ctor.ctor_tag().wrapping_sub(base) as usize;
                    debug_assert!(branch < n as usize);
                    let off = self.code.read_address_at(table + branch * 2);
                    self.code.jump(off);
                }

                opcode::ENCORE => {
                    let clo = self.arena.stack_pop();
                    let arg = self.arena.stack_pop();
                    let addr = clo.closure_addr();
                    let code_ptr = self.arena.heap_read(addr, 1).header_code_ptr();
                    self.self_ref = clo;
                    self.arena.stack_reset();
                    self.arena.stack_push(arg);
                    self.code.jump(code_ptr);
                }

                opcode::HALT => {
                    return Ok(self.arena.stack_peek());
                }

                _ => {
                    return Err(VmError::InvalidOpcode(op));
                }
            }
        }
    }
}
