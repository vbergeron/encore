# Encore VM

A `#![no_std]` bytecode virtual machine for a functional language. Function calls use `ENCORE` (enter a closure with an argument and continuation) and `RETURN` (resume a continuation with a result). Both are tail operations that reset the stack — there is no `CALL`/`RET` pair.

## Value representation

Every runtime value is a **packed 32-bit word**:

```
[payload:16 | meta:8 | typ:8]
```

| Type | `typ` byte | Meta | Payload (high 16 bits) |
|------|-----------|------|------------------------|
| Closure | `0` | `ncap` (capture count) | `HeapAddress` |
| Constructor | `1` | `tag` | `HeapAddress` (or `NULL` if nullary) |
| Closure header | `2` | `0` | `CodeAddress` |
| GC header | `3` | mark bit + 7-bit size | forwarding `HeapAddress` |
| Integer | `4` | upper 24 bits encode a signed integer | (part of the 24-bit value) |

**Integers** use the upper 24 bits as a signed value (approx. \(\pm 8\text{M}\) range). `int_value()` recovers the `i32` via arithmetic right shift.

**`HeapAddress::NULL`** (`0xFFFF`) marks nullary constructors and zero-capture closures that have no heap allocation.

## Memory model

The VM operates on a single `&mut [Value]` buffer split into two regions:

```
[ heap ──────────── hp >  ... free ...  < sp ──────────── stack ]
  grows →                                                 ← grows
```

- **Heap** (`0..hp`): objects allocated by `CLOSURE` and `PACK`, growing upward.
- **Stack** (`sp..len`): evaluation stack, growing downward.
- Allocation fails if `hp + n > sp` after a GC attempt.

### Heap objects

**Closure** (size `2 + ncap`):

| Slot | Content |
|------|---------|
| 0 | `gc_header(2 + ncap)` |
| 1 | `closure_header(code_ptr)` |
| 2.. | captured values |

**Constructor** with arity `k > 0` (size `1 + k`):

| Slot | Content |
|------|---------|
| 0 | `gc_header(1 + k)` |
| 1.. | field values |

Nullary constructors (`k = 0`) are not heap-allocated.

## Registers

| Register | Description |
|----------|-------------|
| `arg` | Current function/continuation argument, accessible via `ARG` |
| `cont` | Current continuation value, accessible via `CONT` |
| `self_ref` | Current closure value, accessible via `SELF` |
| `pc` | Program counter into the bytecode stream |

There are no call frames. `ENCORE` resets the stack and overwrites `arg`, `cont`, `self_ref`, and `pc`. `RETURN` resets the stack and overwrites `arg`, `self_ref`, and `pc`.

## Opcodes

### Data access

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `ARG` | `04` | — | Push `arg` register |
| `SELF` | `05` | — | Push `self_ref` register |
| `CONT` | `0B` | — | Push `cont` register |
| `LOCAL i` | `03 i` | `i: u8` | Push local variable at index `i` |
| `CAPTURE i` | `02 i` | `i: u8` | Push capture slot `i` of current closure |
| `GLOBAL i` | `01 i` | `i: u8` | Push global at index `i` |

### Allocation

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `CLOSURE` | `06` | `addr: u16 LE`, `ncap: u8` | Pop `ncap` values as captures, allocate closure on heap, push closure value |
| `FUNCTION` | `0D` | `addr: u16 LE` | Push a zero-capture closure value with code pointer packed directly in the value (no heap allocation) |
| `PACK tag` | `07 tag` | `tag: u8` | Look up arity from arity table. Pop `arity` values as fields (0 = nullary, no heap alloc), push constructor value |

### Destructuring

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `FIELD i` | `08 i` | `i: u8` | Pop constructor, push its field at index `i` |
| `MATCH` | `09` | `base: u8`, `n: u8`, then `n × u16 LE` jump table | Pop constructor, compute `tag - base`, jump to the corresponding code address |

### Control flow

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `ENCORE` | `0A` | — | Pop closure, pop argument, pop continuation. Set `self_ref`, `arg`, `cont`, reset stack, jump to closure's code pointer. For zero-capture closures (`ncap=0`) the code pointer is read from the value itself; otherwise from the heap |
| `RETURN` | `0C` | — | Pop continuation closure, pop result. Set `self_ref` to the continuation, `arg` to the result, reset stack, jump to continuation's code pointer |
| `FIN` | `00` | — | Halt, return top of stack |

### Integer arithmetic

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `INT` | `10` | 3 bytes LE | Push 24-bit signed integer |
| `INT_ADD` | `11` | — | Pop `b`, pop `a`, push `a + b` |
| `INT_SUB` | `12` | — | Pop `b`, pop `a`, push `a - b` |
| `INT_MUL` | `13` | — | Pop `b`, pop `a`, push `a * b` |
| `INT_EQ` | `14` | — | Pop `b`, pop `a`, push `ctor(1, NULL)` if equal, `ctor(0, NULL)` otherwise |
| `INT_LT` | `15` | — | Pop `b`, pop `a`, push `ctor(1, NULL)` if `a < b`, `ctor(0, NULL)` otherwise |

Comparisons return nullary constructors: tag `1` = true, tag `0` = false.

## Garbage collector

A **mark-compact** (Lisp-2 style) collector runs in-place when the heap cannot satisfy an allocation:

1. **Mark** — trace roots (`arg`, `cont`, `self_ref`, all stack slots) and recursively mark reachable heap objects via `gc_header` mark bits.
2. **Forward** — linear scan computes new addresses for marked objects, stored in the GC header's forwarding field.
3. **Update** — rewrite all pointers (roots, stack, and interior heap pointers) to forwarding addresses.
4. **Compact** — copy marked objects to their new positions and reset `hp`.

## Program binary format

```
Offset  Content
0..4    Magic "ENCR"
4..6    n_arities: u16 LE
6..8    n_globals: u16 LE
8..10   code_len: u16 LE
10..    Arity table (n_arities bytes)
        Global slots (n_globals × 4 bytes, u32 LE each)
        Bytecode (code_len bytes)
```

## Entry points

- **`Vm::new(code, arity_table, globals, mem)`** — create a VM instance.
- **`vm.run()`** — execute from current `pc` until `FIN`.
- **`vm.call(entry, arg)`** — build a thunk closure at `entry`, set `arg`, and run. Supports repeated calls on the same VM instance (GC preserves state between calls).
