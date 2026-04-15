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
        let addr = wl;
        let gc = arena[addr];
        wl = gc.gc_fwd();
        if !arena[addr + 1].is_bytes_hdr() {
            for i in 1..gc.gc_size() as usize {
                enqueue(arena, &mut wl, arena[addr + i]);
            }
        }
    }

    // Phase 2: Compute forwarding addresses
    let new_hp = forward(arena);

    // Phase 3: Update references
    for root in roots.iter_mut() {
        *root = update_value(arena, *root);
    }
    for g in globals.iter_mut() {
        *g = update_value(arena, *g);
    }
    update_heap_refs(arena);

    // Phase 4: Compact
    compact(arena, new_hp);
}

fn enqueue(arena: &mut Arena, wl: &mut HeapAddress, val: Value) {
    if !val.has_heap_addr() { return; }
    let addr = val.heap_addr();
    let gc = arena[addr];
    if gc.gc_is_marked() { return; }
    arena[addr] = gc.gc_set_mark().gc_set_fwd(*wl);
    *wl = addr;
}

/// Linear scan: assign contiguous forwarding addresses to marked objects.
/// Returns the new heap pointer (total size of live data).
fn forward(arena: &mut Arena) -> usize {
    let mut free: usize = 0;
    let mut pos: usize = 0;
    while pos < arena.hp {
        let gc = arena[pos];
        let size = gc.gc_size() as usize;
        if gc.gc_is_marked() {
            arena[pos] = gc.gc_set_fwd(HeapAddress::new(free as u16));
            free += size;
        }
        pos += size;
    }
    free
}

fn update_value(arena: &Arena, val: Value) -> Value {
    if !val.has_heap_addr() { return val; }
    let gc = arena[val.heap_addr()];
    val.with_heap_addr(gc.gc_fwd())
}

/// Rewrite every pointer inside marked heap objects.
fn update_heap_refs(arena: &mut Arena) {
    let mut pos: usize = 0;
    while pos < arena.hp {
        let gc = arena[pos];
        let size = gc.gc_size() as usize;
        if gc.gc_is_marked() && !arena[pos + 1].is_bytes_hdr() {
            for i in 1..size {
                let v = arena[pos + i];
                arena[pos + i] = update_value(arena, v);
            }
        }
        pos += size;
    }
}

/// Slide marked objects to their forwarding addresses and reset hp.
fn compact(arena: &mut Arena, new_hp: usize) {
    let mut pos: usize = 0;
    while pos < arena.hp {
        let gc = arena[pos];
        let size = gc.gc_size() as usize;
        if gc.gc_is_marked() {
            let fwd = gc.gc_fwd().raw() as usize;
            arena[fwd] = Value::gc_header(size as u8);
            for i in 1..size {
                arena[fwd + i] = arena[pos + i];
            }
        }
        pos += size;
    }
    arena.hp = new_hp;
}
