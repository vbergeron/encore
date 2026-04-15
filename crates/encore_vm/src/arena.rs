use crate::error::VmError;
use crate::gc;
#[cfg(feature = "stats")]
use crate::stats::ArenaStats;
use crate::value::{HeapAddress, Value};

pub struct Arena<'a> {
    pub(crate) mem: &'a mut [Value],
    pub(crate) hp: usize,
    #[cfg(feature = "stats")]
    pub(crate) stats: ArenaStats,
}

impl<'a> Arena<'a> {
    pub fn new(mem: &'a mut [Value]) -> Self {
        Self {
            mem,
            hp: 0,
            #[cfg(feature = "stats")]
            stats: ArenaStats::default(),
        }
    }

    pub fn hp(&self) -> usize { self.hp }

    pub fn alloc(
        &mut self,
        n: usize,
        roots: &mut [Value],
        globals: &mut [Value],
    ) -> Result<HeapAddress, VmError> {
        if self.hp + n > self.mem.len() {
            gc::collect(self, roots, globals);
            if self.hp + n > self.mem.len() {
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
        unsafe { *self.mem.get_unchecked(addr.offset(off)) }
    }

    pub fn heap_write(&mut self, addr: HeapAddress, off: usize, val: Value) {
        unsafe { *self.mem.get_unchecked_mut(addr.offset(off)) = val; }
    }
}
