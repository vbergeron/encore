use crate::arena::Arena;
use crate::value::{HeapAddress, Value};

/// Run a Lisp 2 mark-compact garbage collection cycle.
///
/// Phases:
///   1. Mark — trace from roots, set mark bit on live objects
///   2. Forward — assign new (compacted) addresses to marked objects
///   3. Update — rewrite all heap pointers to use forwarding addresses
///   4. Compact — slide live objects down, reset heap pointer
pub fn collect(arena: &mut Arena, self_ref: &mut Value, arg: &mut Value, cont: &mut Value) {
    // Phase 1: Mark
    mark(arena, *self_ref);
    mark(arena, *arg);
    mark(arena, *cont);
    let end = arena.mem.len();
    for i in arena.sp..end {
        mark(arena, arena.mem[i]);
    }

    // Phase 2: Compute forwarding addresses
    let new_hp = forward(arena);

    // Phase 3: Update references
    *self_ref = update_value(arena.mem, *self_ref);
    *arg = update_value(arena.mem, *arg);
    *cont = update_value(arena.mem, *cont);
    for i in arena.sp..end {
        arena.mem[i] = update_value(arena.mem, arena.mem[i]);
    }
    update_heap_refs(arena);

    // Phase 4: Compact
    compact(arena, new_hp);
}

fn mark(arena: &mut Arena, val: Value) {
    if !val.has_heap_addr() { return; }
    let addr = val.heap_addr().raw() as usize;
    let gc = arena.mem[addr];
    if gc.gc_is_marked() { return; }
    arena.mem[addr] = gc.gc_set_mark();
    let size = gc.gc_size() as usize;
    for i in 1..size {
        mark(arena, arena.mem[addr + i]);
    }
}

/// Linear scan: assign contiguous forwarding addresses to marked objects.
/// Returns the new heap pointer (total size of live data).
fn forward(arena: &mut Arena) -> usize {
    let mut free: usize = 0;
    let mut pos: usize = 0;
    while pos < arena.hp {
        let gc = arena.mem[pos];
        let size = gc.gc_size() as usize;
        if gc.gc_is_marked() {
            arena.mem[pos] = gc.gc_set_fwd(HeapAddress::new(free as u16));
            free += size;
        }
        pos += size;
    }
    free
}

fn update_value(mem: &[Value], val: Value) -> Value {
    if !val.has_heap_addr() { return val; }
    let gc = mem[val.heap_addr().raw() as usize];
    val.with_heap_addr(gc.gc_fwd())
}

/// Rewrite every pointer inside marked heap objects.
fn update_heap_refs(arena: &mut Arena) {
    let mut pos: usize = 0;
    while pos < arena.hp {
        let gc = arena.mem[pos];
        let size = gc.gc_size() as usize;
        if gc.gc_is_marked() {
            for i in 1..size {
                arena.mem[pos + i] = update_value(arena.mem, arena.mem[pos + i]);
            }
        }
        pos += size;
    }
}

/// Slide marked objects to their forwarding addresses and reset hp.
fn compact(arena: &mut Arena, new_hp: usize) {
    let mut pos: usize = 0;
    while pos < arena.hp {
        let gc = arena.mem[pos];
        let size = gc.gc_size() as usize;
        if gc.gc_is_marked() {
            let fwd = gc.gc_fwd().raw() as usize;
            arena.mem[fwd] = Value::gc_header(size as u8);
            for i in 1..size {
                arena.mem[fwd + i] = arena.mem[pos + i];
            }
        }
        pos += size;
    }
    arena.hp = new_hp;
}
