/// Monotonic clock supplied by the host (the VM is `no_std`).
/// Expected to return nanoseconds; any monotonic unit works, but
/// [`VmStats`]'s `Display` labels pauses as `ns`.
pub type Clock = fn() -> u64;

pub(crate) fn no_clock() -> u64 { 0 }

#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaStats {
    pub peak_heap: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GcStats {
    /// Number of collections.
    pub count: u64,
    /// Sum of all pause durations, in clock units.
    pub total_pause: u64,
    /// Longest single pause, in clock units.
    pub max_pause: u64,
    /// Total words freed across all collections.
    pub reclaimed: u64,
    /// Live words after the most recent collection.
    pub last_live: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct VmStats {
    pub op_count: u64,
    pub arena: ArenaStats,
    pub gc: GcStats,
}

impl core::fmt::Display for VmStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let word = core::mem::size_of::<crate::value::Value>();
        let gc = &self.gc;
        let avg = if gc.count == 0 { 0 } else { gc.total_pause / gc.count };
        writeln!(f, "ops:          {}", self.op_count)?;
        writeln!(f, "peak_heap:    {} B", self.arena.peak_heap * word)?;
        writeln!(f, "gc_count:     {}", gc.count)?;
        writeln!(f, "gc_pause:     total {} ns, avg {} ns, max {} ns", gc.total_pause, avg, gc.max_pause)?;
        writeln!(f, "gc_reclaimed: {} B", gc.reclaimed as usize * word)?;
        write!(f, "gc_last_live: {} B", gc.last_live * word)
    }
}
