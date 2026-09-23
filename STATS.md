# Runtime statistics

The VM can collect execution statistics: opcode counts, GC activity, and a
time breakdown between the interpreter, the garbage collector and extern
calls. Collection is compiled in only with the `stats` cargo feature.
**Without the feature there is no overhead at all** — the VM's machine code
is identical to a build that has no instrumentation.

## Enabling

| Crate | Feature |
|---|---|
| `encore_vm` | `stats` |
| `encore-toolchain` (CLI) | `stats` (forwards to `encore_vm/stats`) |

### From the CLI

```bash
cargo run --release --features stats --bin encore -- run program.encr --heap-size 1024
```

The report is printed to stderr after the program runs; the result still goes
to stdout. The CLI installs a nanosecond clock based on `std::time::Instant`.
Use `--release` when looking at times: debug builds are several times slower and
distort the GC/mutator split. Shrink `--heap-size` to make the GC run more often.

### As a library

```toml
encore_vm = { version = "0.1", features = ["stats"] }
```

```rust
let mut vm = Vm::init(heap);
vm.set_clock(my_clock);      // optional, see "Clock" below
vm.load(&prog)?;
// ... call_global, etc.
let stats: VmStats = vm.stats();
println!("{stats}");         // or read the fields directly
```

`vm.stats()` returns a snapshot (`VmStats` is `Copy`); counters accumulate
over the whole lifetime of the `Vm`, across `load` and every `call_*`.

## Clock

`encore_vm` is `no_std` and has no time source, so timings use a
host-supplied clock:

```rust
pub type Clock = fn() -> u64;
```

The clock must be monotonic. `Display` labels times as `ns`, so nanoseconds
are the expected unit, but any unit works if you read the fields yourself
(for example CPU cycles). **Without `set_clock`, every time reads 0**; counts,
bytes and opcode statistics are still collected.

Host (std) clock, as used by the CLI:

```rust
fn clock_ns() -> u64 {
    use std::{sync::OnceLock, time::Instant};
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}
vm.set_clock(clock_ns);
```

### QEMU (`examples/eval`, `examples/rbtree`)

The bare-metal examples run on QEMU's `lm3s6965evb` board and already install
a clock from [`examples/common/qemu_clock.rs`](examples/common/qemu_clock.rs):

```rust
qemu_clock::start(cortex_m::Peripherals::take().unwrap().SYST);
let mut vm = boot(HEAP())?;
vm.set_clock(qemu_clock::now_ns);
```

- **Source.** QEMU does not emulate the DWT cycle counter (it always reads 0),
  but it does emulate SysTick. The module runs SysTick as a free-running
  24-bit counter and counts its wraps in the `SysTick` exception handler,
  giving a 64-bit clock.
- **Unit.** QEMU models the board's reset system clock at 12.5 MHz, so one
  tick is 80 ns. The examples never reprogram the clock, so `NS_PER_TICK`
  is a constant.
- **Determinism.** The runners in `.cargo/config.toml` pass
  `-icount shift=0`. QEMU then advances virtual time by exactly 1 ns per guest
  instruction, independent of the host. Reported times read as **guest
  instructions executed**, at 80-instruction resolution, and are identical
  from run to run. Without `-icount`, times follow the host's emulation
  speed and vary between runs.

```bash
cd examples/rbtree && cargo run --release
```

```
run_time:     1526400 ns
  mutator:    1479600 ns (96.9%)
  gc:         46800 ns (3.0%)
  ...
gc_phases:    mark 5120 (10.9%), forward 11520 (24.6%), update 17360 (37.0%), compact 12800 (27.3%)
```

### Real Cortex-M hardware

On Cortex-M3 and later, the DWT cycle counter can serve as the clock. It then
counts cycles, not nanoseconds, and wraps every 2³² cycles.

```rust
// once at startup:
let mut cp = cortex_m::Peripherals::take().unwrap();
cp.DCB.enable_trace();
cp.DWT.enable_cycle_counter();

fn cycles() -> u64 { cortex_m::peripheral::DWT::cycle_count() as u64 }
vm.set_clock(cycles);
```

The clock is read a few times per GC, twice per interpreter entry and twice
per extern call. It is never read per opcode, since reading a clock costs
more than executing an opcode.

## Report

Example: `examples/ackermann` with a 1024-word heap.

```
run_time:     133861 ns
  mutator:    76270 ns (56.9%)
  gc:         57591 ns (43.0%)
  extern:     0 ns (0.0%) over 0 calls
peak_heap:    4096 B
gc_count:     17
gc_pause:     avg 3387 ns, max 4211 ns
gc_phases:    mark 8886 (15.4%), forward 14656 (25.4%), update 18700 (32.4%), compact 15349 (26.6%)
gc_reclaimed: 56408 B
gc_last_live: 1252 B
ops:          32038
  MOV                  6118 (19.0%)
  FIELD                4864 (15.1%)
  PACK                 3738 (11.6%)
  ...
```

### Fields

`VmStats`:

| Field | Report line | Meaning |
|---|---|---|
| `run_time` | `run_time` | Time spent inside the interpreter, measured at the outermost entry only (`load`, `call_*`). Includes GC pauses and extern calls. |
| `mutator_time()` | `mutator` | `run_time − gc.total_pause − extern_time`: time executing bytecode. |
| `extern_calls`, `extern_time` | `extern` | Number of `EXTERN` executions and time inside host handlers. |
| `arena.peak_heap` | `peak_heap` | High-water mark of the heap pointer. |
| `op_count` | `ops` | Total opcodes executed. |
| `op_counts[op]` | lines under `ops` | Executions per opcode byte (`0x00..0x40`, see `opcode::OPCODE_SLOTS`). Only nonzero entries are printed, most frequent first. |

`VmStats::gc` (`GcStats`):

| Field | Report line | Meaning |
|---|---|---|
| `count` | `gc_count` | Number of collections. |
| `total_pause`, `max_pause` | `gc`, `gc_pause` | Sum and maximum of pause durations; the average is `total_pause / count`. |
| `phases.{mark,forward,update,compact}` | `gc_phases` | Cumulative time per collector phase (see [VM.md](VM.md#garbage-collector)). Sums to `total_pause`. |
| `reclaimed` | `gc_reclaimed` | Total words freed across all collections. |
| `last_live` | `gc_last_live` | Live words after the most recent collection. |

The struct fields hold sizes in words (4 bytes each); the report prints bytes.

### Reading the GC phases

Mark cost grows with the amount of **live** data. Forward, update and compact
each walk the **entire used heap**, dead objects included. A low mark share
with large forward/update/compact shares means the collector spends most of
its time passing over garbage.

## Caveats

- **Re-entrancy.** When an extern handler calls back into the VM, the nested
  run is counted as extern time, not as run time as well. The VM does not
  collect garbage while an extern is executing, so GC time never overlaps
  extern time.
- **GC outside the interpreter.** Host-side allocations (`alloc_ctor`,
  `alloc_bytes`, argument encoding for `call_global`) can trigger a
  collection outside any interpreter run. Its pause counts in `gc` but not in
  `run_time`. `mutator_time()` saturates at zero, but the percentages can be
  slightly off in that case.
- **Memory.** With `stats`, `VmStats` (about 600 B: 64 opcode counters plus
  timings) lives inside `Vm`, and `vm.stats()` returns a copy. On small
  targets, count it in your stack budget: `Vm` is often moved by value, for
  example out of `boot()`.
- **API change under the feature.** `gc::collect` takes two extra
  parameters (`&mut GcStats`, `Clock`) when `stats` is enabled.

## Adding instrumentation

The zero-overhead guarantee depends on code being **compiled out**, never
skipped by a runtime check. Adding a branch or an `Option<Clock>` in the hot
path would cost something even with the feature off.

- **Statements:** wrap them in the crate-internal `stat!` macro, which
  expands to nothing without `stats`. Bindings made inside `stat!` are visible
  to later `stat!` blocks in the same scope.

  ```rust
  stat! { let t0 = (self.clock)(); }
  let result = f(self, arg);
  stat! { self.stats.extern_time += (self.clock)().saturating_sub(t0); }
  ```

- **Struct fields and function parameters:** use
  `#[cfg(feature = "stats")]` directly. Rust allows it on individual
  parameters, which keeps signatures unchanged without the feature, as in
  `gc::collect`.
- **Types and helpers** live in `crates/encore_vm/src/stats.rs`, which is
  compiled only with the feature. Anything else used only by stats, such as
  `opcode::name`, must be gated too.

### Verifying zero overhead

Compare the VM machine code of a release build without the feature against the
base commit:

```bash
vm_asm() {
  objdump -d --no-show-raw-insn "$1" \
    | awk '/<.*encore_vm.*>:$/{p=1} /^$/{p=0} p' \
    | sed -E 's/^ *[0-9a-f]+:\s*//; s/[0-9a-f]{6,}//g' | md5sum
}
git worktree add /tmp/base <base-commit>
(cd /tmp/base && cargo build --release -p encore-toolchain)
cargo build --release -p encore-toolchain
vm_asm /tmp/base/target/release/encore
vm_asm target/release/encore     # hashes must match
```

The `sed` step strips instruction addresses so that moved code still compares
equal.
