use crate::arena::Arena;
use crate::value::{HeapAddress, Value};

/// Run a Lisp 2 mark-compact garbage collection cycle.
///
/// Phases:
///   1. Mark — trace from roots, set mark bit on live objects
///   2. Forward — assign new (compacted) addresses to marked objects
///   3. Update — rewrite all heap pointers to use forwarding addresses
///   4. Compact — slide live objects down, reset heap pointer
pub fn collect(arena: &mut Arena, roots: &mut [Value], globals: &mut [Value]) {
    // Phase 1: Mark (iterative, using the fwd field as an intrusive worklist)
    let mut wl = HeapAddress::NULL;
    for root in roots.iter() {
        enqueue(arena, &mut wl, *root);
    }
    for g in globals.iter() {
        enqueue(arena, &mut wl, *g);
    }
    while !wl.is_null() {
        let addr = wl.raw() as usize;
        let gc = arena.mem[addr];
        wl = gc.gc_fwd();
        for i in 1..gc.gc_size() as usize {
            enqueue(arena, &mut wl, arena.mem[addr + i]);
        }
    }

    // Phase 2: Compute forwarding addresses
    let new_hp = forward(arena);

    // Phase 3: Update references
    for root in roots.iter_mut() {
        *root = update_value(arena.mem, *root);
    }
    for g in globals.iter_mut() {
        *g = update_value(arena.mem, *g);
    }
    update_heap_refs(arena);

    // Phase 4: Compact
    compact(arena, new_hp);
}

fn enqueue(arena: &mut Arena, wl: &mut HeapAddress, val: Value) {
    if !val.has_heap_addr() { return; }
    let addr = val.heap_addr().raw() as usize;
    let gc = arena.mem[addr];
    if gc.gc_is_marked() { return; }
    arena.mem[addr] = gc.gc_set_mark().gc_set_fwd(*wl);
    *wl = HeapAddress::new(addr as u16);
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
