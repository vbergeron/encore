use crate::error::VmError;
use crate::gc;
#[cfg(feature = "stats")]
use crate::stats::ArenaStats;
use crate::value::{HeapAddress, Value};

pub struct Arena<'a> {
    pub(crate) mem: &'a mut [Value],
    pub(crate) hp: usize,
    pub(crate) sp: usize,
    stack_floor: usize,
    #[cfg(feature = "stats")]
    pub(crate) stats: ArenaStats,
}

impl<'a> Arena<'a> {
    pub fn new(mem: &'a mut [Value]) -> Self {
        let sp = mem.len();
        Self {
            mem,
            hp: 0,
            sp,
            stack_floor: sp,
            #[cfg(feature = "stats")]
            stats: ArenaStats::default(),
        }
    }

    pub fn hp(&self) -> usize { self.hp }

    // -- Heap (grows up from 0) --

    pub fn alloc(
        &mut self,
        n: usize,
        roots: &mut [Value],
        globals: &mut [Value],
    ) -> Result<HeapAddress, VmError> {
        let limit = self.stack_floor.min(self.sp);
        if self.hp + n > limit {
            gc::collect(self, roots, globals);
            let limit = self.stack_floor.min(self.sp);
            if self.hp + n > limit {
                return Err(VmError::HeapOverflow);
            }
        }
        let addr = HeapAddress::new(self.hp as u16);
        self.hp += n;
        #[cfg(feature = "stats")]
        if self.hp > self.stats.peak_heap { self.stats.peak_heap = self.hp; }
        Ok(addr)
    }

    pub fn heap_read(&self, addr: HeapAddress, off: usize) -> Value {
        self.mem[addr.offset(off)]
    }

    pub fn heap_write(&mut self, addr: HeapAddress, off: usize, val: Value) {
        self.mem[addr.offset(off)] = val;
    }

    // -- Stack (grows down from end) --

    pub fn stack_reserve(
        &mut self,
        sd: usize,
        roots: &mut [Value],
        globals: &mut [Value],
    ) -> Result<(), VmError> {
        if sd > self.sp || self.sp - sd < self.hp {
            gc::collect(self, roots, globals);
            if sd > self.sp || self.sp - sd < self.hp {
                return Err(VmError::StackOverflow);
            }
        }
        self.stack_floor = self.sp - sd;
        Ok(())
    }

    pub fn stack_push(&mut self, val: Value) {
        self.sp -= 1;
        self.mem[self.sp] = val;
        #[cfg(feature = "stats")]
        {
            let depth = self.mem.len() - self.sp;
            if depth > self.stats.peak_stack { self.stats.peak_stack = depth; }
        }
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
        self.stack_floor = self.sp;
    }
}
