use crate::error::VmError;
use crate::gc;
use crate::value::{HeapAddress, Value};

pub struct Arena<'a> {
    pub(crate) mem: &'a mut [Value],
    pub(crate) hp: usize,
    pub(crate) sp: usize,
}

impl<'a> Arena<'a> {
    pub fn new(mem: &'a mut [Value]) -> Self {
        let sp = mem.len();
        Self { mem, hp: 0, sp }
    }

    pub fn hp(&self) -> usize { self.hp }

    fn overflowing(&self, n: usize) -> bool {
        self.hp + n > self.sp
    }

    // -- Heap (grows up from 0) --

    pub fn alloc(
        &mut self,
        n: usize,
        self_ref: &mut Value,
        arg: &mut Value,
        cont: &mut Value,
    ) -> Result<HeapAddress, VmError> {
        if self.overflowing(n) {
            gc::collect(self, self_ref, arg, cont);
            if self.overflowing(n) {
                return Err(VmError::HeapOverflow);
            }
        }
        let addr = HeapAddress::new(self.hp as u16);
        self.hp += n;
        Ok(addr)
    }

    pub fn heap_read(&self, addr: HeapAddress, off: usize) -> Value {
        self.mem[addr.offset(off)]
    }

    pub fn heap_write(&mut self, addr: HeapAddress, off: usize, val: Value) {
        self.mem[addr.offset(off)] = val;
    }

    // -- Stack (grows down from end) --

    pub fn stack_ensure(&self, n: usize) -> Result<(), VmError> {
        if self.sp < self.hp + n {
            return Err(VmError::StackOverflow);
        }
        Ok(())
    }

    pub fn stack_push(&mut self, val: Value) {
        self.sp -= 1;
        self.mem[self.sp] = val;
    }

    pub fn stack_pop(&mut self) -> Value {
        let val = self.mem[self.sp];
        self.sp += 1;
        val
    }

    pub fn stack_peek(&self) -> Value {
        self.mem[self.sp]
    }

    pub fn stack_local(&self, idx: u8) -> Value {
        self.mem[self.mem.len() - 1 - idx as usize]
    }

    pub fn stack_reset(&mut self) {
        self.sp = self.mem.len();
    }
}
