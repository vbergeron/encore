# Encore VM

A `#![no_std]` bytecode virtual machine for a functional language. Function calls use a single `ENCORE` opcode that sets `SELF`, `CONT`, resolves the code pointer, and jumps — it never returns. Continuation resumption is an `ENCORE` where the continuation register holds the `NULL` sentinel. There is no `CALL`/`RET` pair and no call stack.

## Value representation

Every runtime value is a **packed 32-bit word**:

```
[payload:16 | meta:8 | typ:8]
```

| Type | `typ` byte | Meta | Payload (high 16 bits) |
|------|-----------|------|------------------------|
| Closure | `0` | — | `HeapAddress` |
| Constructor | `1` | `tag` | `HeapAddress` (or `NULL` if nullary) |
| Closure header | `2` | `env_len` (capture count) | `CodeAddress` |
| GC header | `3` | mark bit + 7-bit size | forwarding `HeapAddress` |
| Integer | `4` | upper 24 bits encode a signed integer | (part of the 24-bit value) |
| Function | `5` | — | `CodeAddress` |
| Bytes | `6` | — | `HeapAddress` |
| Bytes header | `7` | byte length (24-bit, upper 24 bits) | (part of the 24-bit length) |

**Functions** (`TYP_FUNC`) are bare function values with no captures. The code pointer is stored inline — no heap allocation needed. **Closures** (`TYP_CLOS`) always point to a heap object whose header carries `env_len` (capture count) and `code_ptr`.

**Integers** use the upper 24 bits as a signed value (approx. ±8M range). `int_value()` recovers the `i32` via arithmetic right shift.

**`HeapAddress::NULL`** (`0xFFFF`) marks nullary constructors that have no heap allocation.

## Memory model

The VM operates on a single `&mut [Value]` buffer used as a **heap arena**:

```
[ heap ──────────── hp >  ... free ... ]
  grows →
```

- **Heap** (`0..hp`): objects allocated by `CLOSURE`, `PACK`, and byte-string opcodes, growing upward via bump allocation.
- Allocation fails with `HeapOverflow` if `hp + n > mem.len()` after a GC attempt.

### Heap objects

**Closure** (size `2 + env_len`):

| Slot | Content |
|------|---------|
| 0 | `gc_header(2 + env_len)` |
| 1 | `closure_header(env_len, code_ptr)` |
| 2.. | captured values |

**Constructor** with arity `k > 0` (size `1 + k`):

| Slot | Content |
|------|---------|
| 0 | `gc_header(1 + k)` |
| 1.. | field values |

Nullary constructors (`k = 0`) are not heap-allocated.

**Byte string** (size `2 + ceil(len/4)`):

| Slot | Content |
|------|---------|
| 0 | `gc_header(2 + n_data_words)` |
| 1 | `bytes_header(byte_len)` |
| 2.. | packed byte data (4 bytes per word, little-endian) |

## Registers

The VM has a **256-register file** indexed by `Reg(u8)`. Registers are accessed with unchecked indexing since `u8` is always in bounds for a 256-element array.

| Register | Index | Description |
|----------|-------|-------------|
| `SELF` | `0` | Current function/closure value |
| `CONT` | `1` | Current continuation value |
| `A1`–`A8` | `2`–`9` | Argument passing |
| `X01`+ | `10`+ | Local variables |
| `NULL` | `0xFF` | Sentinel — initialized to `Value::function(0xFFFF)` |

There are no call frames. `ENCORE` overwrites `SELF` and `CONT`, then jumps to the callee's code pointer. Arguments are staged into `A1`–`A8` by `MOV` instructions before `ENCORE`.

## Opcodes

All opcodes use a **register-based** format. Operands are register indices (`u8`), not stack slots.

### Data movement

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `MOV` | `01` | `rd: Reg`, `rs: Reg` | `regs[rd] = regs[rs]` |
| `CAPTURE` | `02` | `rd: Reg`, `idx: u8` | `regs[rd] = heap[SELF.closure_addr() + 2 + idx]` |
| `GLOBAL` | `03` | `rd: Reg`, `idx: u8` | `regs[rd] = globals[idx]` |

### Allocation

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `CLOSURE` | `06` | `rd: Reg`, `addr: u16 LE`, `ncap: u8`, `cap₁..capₙ: Reg...` | Allocate closure on heap with `ncap` captured values read from registers, store closure value in `rd` |
| `FUNCTION` | `0D` | `rd: Reg`, `addr: u16 LE` | Store `TYP_FUNC` value with code pointer directly in `rd` (no heap allocation) |
| `PACK` | `07` | `rd: Reg`, `tag: u8`, `f₁..fₙ: Reg...` | Look up arity from arity table. Read `arity` field values from registers (0 = nullary, no heap alloc), store constructor value in `rd` |

### Destructuring

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `FIELD` | `08` | `rd: Reg`, `rs: Reg`, `idx: u8` | `regs[rd] = heap[regs[rs].ctor_addr() + 1 + idx]` |
| `UNPACK` | `0E` | `rd: Reg`, `tag: u8`, `rs: Reg` | For `i in 0..arity_table[tag]`: `regs[rd + i] = heap[regs[rs].ctor_addr() + 1 + i]` |
| `MATCH` | `09` | `rs: Reg`, `base: u8`, `n: u8`, then `n × u16 LE` jump table | Compute `regs[rs].ctor_tag() - base`, jump to the corresponding code address |
| `BRANCH` | `0B` | `rs: Reg`, `base: u8`, `addr₀: u16 LE`, `addr₁: u16 LE` | Two-way match: jump to `addr₀` if tag equals `base`, otherwise jump to `addr₁` |

### Control flow

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `ENCORE` | `0A` | `rf: Reg`, `rk: Reg` | Set `SELF = regs[rf]`, `CONT = regs[rk]`, resolve code pointer from `rf` (inline for `TYP_FUNC`, from heap header for `TYP_CLOS`), jump |
| `FIN` | `00` | `rs: Reg` | Halt, return `regs[rs]` |

Arguments `A1`–`A8` are staged by the compiler via `MOV` instructions before `ENCORE`. The `NULL` register (`0xFF`) serves as a dead continuation when resuming a continuation via `ENCORE`.

### Integer operations

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `INT` | `10` | `rd: Reg`, `3 bytes LE` | `regs[rd] = Value::int(sign_extend_24(bytes))` |
| `INT_0` | `18` | `rd: Reg` | `regs[rd] = Value::int(0)` |
| `INT_1` | `19` | `rd: Reg` | `regs[rd] = Value::int(1)` |
| `INT_2` | `1A` | `rd: Reg` | `regs[rd] = Value::int(2)` |
| `INT_ADD` | `11` | `rd: Reg`, `ra: Reg`, `rb: Reg` | `regs[rd] = int(regs[ra] + regs[rb])` (wrapping) |
| `INT_SUB` | `12` | `rd: Reg`, `ra: Reg`, `rb: Reg` | `regs[rd] = int(regs[ra] - regs[rb])` (wrapping) |
| `INT_MUL` | `13` | `rd: Reg`, `ra: Reg`, `rb: Reg` | `regs[rd] = int(regs[ra] * regs[rb])` (wrapping) |
| `INT_EQ` | `14` | `rd: Reg`, `ra: Reg`, `rb: Reg` | `regs[rd] = ctor(1, NULL)` if equal, `ctor(0, NULL)` otherwise |
| `INT_LT` | `15` | `rd: Reg`, `ra: Reg`, `rb: Reg` | `regs[rd] = ctor(1, NULL)` if `a < b`, `ctor(0, NULL)` otherwise |
| `INT_BYTE` | `16` | `rd: Reg`, `rs: Reg` | Convert integer 0–255 to a single-byte `Bytes` value; error if out of range |

Comparisons return nullary constructors: tag `1` = true, tag `0` = false.

### Byte string operations

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `BYTES` | `30` | `rd: Reg`, `len: u8`, `data: len bytes` | Allocate byte string from inline data |
| `BYTES_LEN` | `31` | `rd: Reg`, `rs: Reg` | `regs[rd] = int(byte_length(regs[rs]))` |
| `BYTES_GET` | `32` | `rd: Reg`, `rs: Reg`, `ri: Reg` | `regs[rd] = int(byte_at(regs[rs], regs[ri]))` |
| `BYTES_CONCAT` | `33` | `rd: Reg`, `ra: Reg`, `rb: Reg` | `regs[rd] = concat(regs[ra], regs[rb])` |
| `BYTES_SLICE` | `34` | `rd: Reg`, `rs: Reg`, `ri: Reg`, `rn: Reg` | `regs[rd] = slice(regs[rs], start=regs[ri], len=regs[rn])` |
| `BYTES_EQ` | `35` | `rd: Reg`, `ra: Reg`, `rb: Reg` | `regs[rd] = ctor(1, NULL)` if equal, `ctor(0, NULL)` otherwise |

### Foreign functions

| Opcode | Hex | Operands | Effect |
|--------|-----|----------|--------|
| `EXTERN` | `20` | `rd: Reg`, `ra: Reg`, `slot: u16 LE` | `regs[rd] = extern_fns[slot](regs[ra])` |

Up to 32 extern slots are available. Extern functions have signature `fn(Value) -> Value`.

## Garbage collector

A **mark-compact** (Lisp-2 style) collector runs in-place when the heap cannot satisfy an allocation:

1. **Mark** — trace roots (the full 256-register file and all globals) and iteratively mark reachable heap objects via `gc_header` mark bits. Byte-string payloads are not traced as pointers.
2. **Forward** — linear scan computes new addresses for marked objects, stored in the GC header's forwarding field.
3. **Update** — rewrite all pointers (roots and interior heap pointers) to forwarding addresses.
4. **Compact** — slide marked objects to their new positions and reset `hp`.

## Program binary format

```
Offset  Content
0..4    Magic "ENCR"
4..6    n_arities: u16 LE
6..8    n_globals: u16 LE
8..10   code_len: u16 LE
10..    Arity table (n_arities bytes)
        Global slots (n_globals × 2 bytes: u16 LE code offset)
        Bytecode (code_len bytes)
```

Optional metadata may be appended after the bytecode:

```
Section 1 — constructor names:
  n_ctors: u16 LE
  For each: tag: u8, name_len: u8, name: name_len bytes (UTF-8)

Section 2 — global/define names:
  n_globals: u16 LE
  For each: idx: u8, name_len: u8, name: name_len bytes (UTF-8)
```

## Entry points

- **`Vm::init(mem)`** — create a VM instance with a heap arena.
- **`vm.load(&prog)`** — parse a program binary, initialize globals by running each define's thunk.
- **`vm.call(global_idx, arg)`** — call a global function with an argument, return the result.
- **`vm.call_value(func, arg)`** — call an arbitrary function value with an argument.
- **`vm.register_extern(slot, f)`** — register a host function at a given extern slot.
