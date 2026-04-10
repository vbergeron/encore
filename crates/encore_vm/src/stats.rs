#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaStats {
    pub peak_heap: usize,
    pub peak_stack: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VmStats {
    pub op_count: u64,
    pub arena: ArenaStats,
}

impl core::fmt::Display for VmStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "ops:        {}", self.op_count)?;
        writeln!(f, "peak_heap:  {} words", self.arena.peak_heap)?;
        write!(f, "peak_stack: {} words", self.arena.peak_stack)
    }
}
