/// Monotonic clock supplied by the host (the VM is `no_std`).
/// Expected to return nanoseconds; any monotonic unit works, but
/// [`VmStats`]'s `Display` labels times as `ns`.
pub type Clock = fn() -> u64;

pub(crate) fn no_clock() -> u64 { 0 }

#[derive(Clone, Copy, Debug, Default)]
pub struct ArenaStats {
    pub peak_heap: usize,
}

/// Time spent in each phase of the mark-compact collector, in clock units.
#[derive(Clone, Copy, Debug, Default)]
pub struct GcPhases {
    pub mark: u64,
    pub forward: u64,
    pub update: u64,
    pub compact: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GcStats {
    /// Number of collections.
    pub count: u64,
    /// Sum of all pause durations, in clock units.
    pub total_pause: u64,
    /// Longest single pause, in clock units.
    pub max_pause: u64,
    /// Cumulative time per phase; sums to `total_pause`.
    pub phases: GcPhases,
    /// Total words freed across all collections.
    pub reclaimed: u64,
    /// Live words after the most recent collection.
    pub last_live: usize,
}

impl GcStats {
    /// Record one collection given clock readings at each phase boundary.
    pub(crate) fn record(&mut self, t: [u64; 5], hp_before: usize, hp_after: usize) {
        let d = |i: usize| t[i + 1].saturating_sub(t[i]);
        let pause = t[4].saturating_sub(t[0]);
        self.count += 1;
        self.total_pause += pause;
        if pause > self.max_pause { self.max_pause = pause; }
        self.phases.mark += d(0);
        self.phases.forward += d(1);
        self.phases.update += d(2);
        self.phases.compact += d(3);
        self.reclaimed += (hp_before - hp_after) as u64;
        self.last_live = hp_after;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VmStats {
    pub op_count: u64,
    /// Executions per opcode byte.
    pub op_counts: [u64; 256],
    /// Time inside the interpreter loop (outermost entry only), in clock
    /// units. Includes GC pauses and extern calls that happen during it.
    pub run_time: u64,
    pub extern_calls: u64,
    /// Time inside extern handlers, in clock units.
    pub extern_time: u64,
    pub arena: ArenaStats,
    pub gc: GcStats,
}

impl Default for VmStats {
    fn default() -> Self {
        Self {
            op_count: 0,
            op_counts: [0; 256],
            run_time: 0,
            extern_calls: 0,
            extern_time: 0,
            arena: ArenaStats::default(),
            gc: GcStats::default(),
        }
    }
}

impl VmStats {
    /// Interpreter time excluding GC and externs.
    pub fn mutator_time(&self) -> u64 {
        self.run_time
            .saturating_sub(self.gc.total_pause)
            .saturating_sub(self.extern_time)
    }
}

/// `part / whole` as a percentage with one decimal, without floats.
struct Pct(u64, u64);

impl core::fmt::Display for Pct {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let permille = if self.1 == 0 { 0 } else { self.0 as u128 * 1000 / self.1 as u128 };
        write!(f, "{}.{}%", permille / 10, permille % 10)
    }
}

impl core::fmt::Display for VmStats {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let word = core::mem::size_of::<crate::value::Value>();
        let gc = &self.gc;
        let ph = &gc.phases;
        let avg = if gc.count == 0 { 0 } else { gc.total_pause / gc.count };
        writeln!(f, "run_time:     {} ns", self.run_time)?;
        writeln!(f, "  mutator:    {} ns ({})", self.mutator_time(), Pct(self.mutator_time(), self.run_time))?;
        writeln!(f, "  gc:         {} ns ({})", gc.total_pause, Pct(gc.total_pause, self.run_time))?;
        writeln!(f, "  extern:     {} ns ({}) over {} calls", self.extern_time, Pct(self.extern_time, self.run_time), self.extern_calls)?;
        writeln!(f, "peak_heap:    {} B", self.arena.peak_heap * word)?;
        writeln!(f, "gc_count:     {}", gc.count)?;
        writeln!(f, "gc_pause:     avg {} ns, max {} ns", avg, gc.max_pause)?;
        writeln!(
            f,
            "gc_phases:    mark {} ({}), forward {} ({}), update {} ({}), compact {} ({})",
            ph.mark, Pct(ph.mark, gc.total_pause),
            ph.forward, Pct(ph.forward, gc.total_pause),
            ph.update, Pct(ph.update, gc.total_pause),
            ph.compact, Pct(ph.compact, gc.total_pause),
        )?;
        writeln!(f, "gc_reclaimed: {} B", gc.reclaimed as usize * word)?;
        writeln!(f, "gc_last_live: {} B", gc.last_live * word)?;
        write!(f, "ops:          {}", self.op_count)?;
        // Nonzero opcodes, most frequent first (selection over 256 slots; no alloc).
        let mut last = u64::MAX;
        let mut last_op = 0usize;
        loop {
            let mut best: Option<(usize, u64)> = None;
            for (op, &n) in self.op_counts.iter().enumerate() {
                let below = n < last || (n == last && op > last_op);
                if n > 0 && below && best.map_or(true, |(_, b)| n > b) {
                    best = Some((op, n));
                }
            }
            let Some((op, n)) = best else { break };
            write!(f, "\n  {:<12} {:>12} ({})", crate::opcode::name(op as u8), n, Pct(n, self.op_count))?;
            last = n;
            last_op = op;
        }
        Ok(())
    }
}
