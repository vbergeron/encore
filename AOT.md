# Encore CPS → ARMv7 Thumb-2 AOT Compilation

This document details the strategy for ahead-of-time compilation of the Encore
CPS intermediate representation to native ARMv7 (Thumb-2) machine code.

## 1. Value Representation

The existing 32-bit packed value representation is preserved unchanged:

```
  31        16 15     8 7      0
  ┌──────────┬────────┬────────┐
  │ payload  │  meta  │  typ   │
  └──────────┴────────┴────────┘
```

| Type     | typ | meta      | payload        |
|----------|-----|-----------|----------------|
| Function | 5   | 0         | code table idx |
| Closure  | 0   | 0         | heap address   |
| Ctor     | 1   | tag (u8)  | heap address   |
| Integer  | 4   | low bits  | high bits      |
| GC hdr   | 3   | mark+size | fwd address    |
| Clos hdr | 2   | env_len   | code table idx |

Integers are 24-bit sign-extended: `int_value = (raw as i32) >> 8`.

The key change for AOT is the **code table index**: in the bytecode VM a
`CodeAddress(u16)` is a byte offset into the bytecode stream. In AOT mode it
becomes an index into a table of native function pointers, described below.

## 2. Register Mapping

### 2.1. VM virtual registers

The Encore VM has 32 virtual registers:

| VM reg   | Index | Role                      |
|----------|-------|---------------------------|
| SELF     | 0     | Current function/closure   |
| CONT     | 1     | Current continuation       |
| A1–A8    | 2–9   | Argument passing           |
| X01–X22  | 10–31 | Locals                     |
| NULL     | 0xFF  | Reads as `0x0000_FFFF`     |

### 2.2. ARM register assignment

ARMv7 has 16 registers (r0–r15), of which r13 (SP), r14 (LR), r15 (PC) are
reserved. That leaves r0–r12 for our use.

Proposed pinning:

| ARM reg | Alias    | Encore role              |
|---------|----------|--------------------------|
| r0–r1   | scratch  | Temporaries for emission  |
| r2      | vA1      | A1 — first argument       |
| r3      | vA2      | A2 — second argument      |
| r4      | vSELF    | SELF                      |
| r5      | vCONT    | CONT                      |
| r6      | vX01     | First local               |
| r7      | vX02     | Second local              |
| r8      | vX03     | Third local               |
| r9      | vX04     | Fourth local              |
| r10     | vRF      | Register file base ptr    |
| r11     | vCT      | Code table base ptr       |
| r12     | vHP      | Heap pointer (bump alloc) |

- **r10 (vRF)**: points to a `[u32; 32]` array in memory — the **shadow
  register file**. VM registers that don't fit in ARM registers (A3–A8 and
  X05–X22) are accessed as `LDR/STR [r10, #vreg*4]`.
- **r11 (vCT)**: points to the code table `[fn_ptr; N]`.  Used to resolve code
  pointers during ENCORE.
- **r12 (vHP)**: the current heap bump pointer. Points into the arena. The heap
  limit is stored in a known memory location nearby.

### 2.3. Spilling strategy

When the emitter encounters a virtual register `Xn` with `n >= 5`, the access
is lowered as a load/store from the shadow register file:

```arm
@ Read virtual register X07 (index 16) into r0
LDR   r0, [r10, #64]       @ 16 * 4 = 64

@ Write r0 to virtual register X07
STR   r0, [r10, #64]
```

For the pinned registers (SELF, CONT, A1, A2, X01–X04), the ARM register is
used directly with no memory access.

Arguments A3–A8 are spilled since they're used less frequently (most Encore
functions are unary or binary after CPS transform + contification). The emitter
stages all arguments into the right location (ARM reg or shadow slot) before
ENCORE.

## 3. Code Pointers and the Code Table

### 3.1. Problem

In the bytecode VM, `Value::function(code_ptr)` stores a 16-bit offset into the
bytecode buffer. In native code, function entry points are 32-bit addresses that
don't fit in the 16-bit payload.

### 3.2. Solution: indexed code table

All emitted function bodies are assigned a sequential **code table index** (u16).
A global array holds the native addresses:

```
code_table: [*const u8; N]   // indexed by u16, max 65536 entries
```

`Value::function` and `Value::closure_header` store the code table index in
their payload field, exactly as they do today.

To resolve a code table index to a native address:

```arm
@ r0 = code_table_index (u16, already extracted from value)
LDR   r0, [r11, r0, LSL #2]   @ r0 = code_table[idx]
BX    r0                        @ jump to native code
```

This adds one load per indirect call. The table is small and hot in L1.

### 3.3. Closure code pointer resolution

For closures the code table index is stored in the closure header on the heap.
Resolution:

```arm
@ r0 = closure value (TYP_CLOS, heap addr in upper 16)
LSR   r1, r0, #16              @ r1 = heap address
LDR   r1, [heap_base, r1, LSL #2]  @ read closure header (slot 1... see below)
@ actually: offset +4 from heap object start
LSR   r1, r1, #16              @ r1 = code table index from header
LDR   r1, [r11, r1, LSL #2]    @ r1 = native address
BX    r1
```

## 4. Heap: Allocation, Layout and GC

### 4.1. Arena layout

The heap is a contiguous `[u32; HEAP_SIZE]` array (each slot is one `Value`).
The bump pointer `hp` (held in r12 at runtime) advances upward.  A heap limit
address is stored at a fixed location (e.g. `[r10, #-4]`, just below the shadow
register file).

### 4.2. Allocation (bump pointer)

Every allocation site (CLOSURE and PACK with arity > 0) emits:

```arm
@ Allocate `size` words. r12 = current hp.
ADD   r0, r12, #(size * 4)     @ new hp
LDR   r1, [r10, #-4]           @ heap limit
CMP   r0, r1
BHI   .gc_needed
MOV   r1, r12                  @ r1 = object address (old hp)
MOV   r12, r0                  @ bump hp
@ ... write header + fields to [r1] ...
B     .alloc_done

.gc_needed:
BL    _encore_gc               @ call GC runtime (see below)
B     .retry                   @ retry allocation after GC
.alloc_done:
```

### 4.3. Object layout

Closures:

```
heap[addr+0] = gc_header(2 + ncap)   @ GC header: [TYP_GC | size | fwd]
heap[addr+1] = closure_header(ncap, code_idx)  @ [TYP_HDR | env_len | code_idx]
heap[addr+2] = capture_0
heap[addr+3] = capture_1
...
```

Constructors (arity > 0):

```
heap[addr+0] = gc_header(1 + arity)  @ GC header
heap[addr+1] = field_0
heap[addr+2] = field_1
...
```

Nullary constructors have no heap allocation: `Value::ctor(tag, HeapAddress::NULL)`.

### 4.4. GC integration

The existing GC is a Lisp-2 mark-compact collector with four phases:

1. **Mark** — trace from roots (registers + globals), set mark bit in GC headers
2. **Forward** — linear scan, assign contiguous forwarding addresses
3. **Update** — rewrite all heap pointers (in roots and in heap objects)
4. **Compact** — slide live objects down, reset hp

For AOT, the GC routine is a Rust/C function callable from generated code. At
a GC safepoint (= every allocation site), the generated code must make roots
visible:

```arm
_encore_gc:
    @ Flush pinned registers to shadow register file
    STR   r4,  [r10, #0]       @ SELF  -> slot 0
    STR   r5,  [r10, #4]       @ CONT  -> slot 1
    STR   r2,  [r10, #8]       @ A1    -> slot 2
    STR   r3,  [r10, #12]      @ A2    -> slot 3
    STR   r6,  [r10, #40]      @ X01   -> slot 10
    STR   r7,  [r10, #44]      @ X02   -> slot 11
    STR   r8,  [r10, #48]      @ X03   -> slot 12
    STR   r9,  [r10, #52]      @ X04   -> slot 13
    @ Store current hp
    STR   r12, [r10, #-8]      @ save hp for the GC
    @ Call into Rust: gc_collect(shadow_regs, globals, arena)
    MOV   r0, r10
    BL    _gc_collect_native
    @ Reload pinned registers (GC may have moved heap pointers)
    LDR   r4,  [r10, #0]
    LDR   r5,  [r10, #4]
    LDR   r2,  [r10, #8]
    LDR   r3,  [r10, #12]
    LDR   r6,  [r10, #40]
    LDR   r7,  [r10, #44]
    LDR   r8,  [r10, #48]
    LDR   r9,  [r10, #52]
    LDR   r12, [r10, #-8]      @ reload hp
    BX    LR
```

The Rust-side GC function receives the shadow register file as its root array,
identical to how `Vm::alloc` passes `&mut self.regs` today.

## 5. Opcode-by-Opcode Translation Strategy

### 5.1. MOV — register copy

**VM**: `regs[rd] = read_reg(rs)`

```arm
@ If both rd and rs are pinned ARM registers: single MOV
MOV   rd_arm, rs_arm

@ If rs is spilled (e.g. X07):
LDR   r0, [r10, #offset_rs]
@ then store to rd (pinned or spilled)
STR   r0, [r10, #offset_rd]   @ or MOV rd_arm, r0

@ If rs == NULL (0xFF): materialize the null sentinel
MOV   r0, #0xFFFF             @ MOVW r0, #0xFFFF
@ store to rd
```

### 5.2. CAPTURE — load from closure environment

**VM**: `regs[rd] = heap[SELF.closure_addr() + 2 + idx]`

```arm
@ r4 = SELF (closure value)
LSR   r0, r4, #16              @ r0 = heap address (word index)
ADD   r0, r0, #(2 + idx)       @ offset: gc_hdr + clos_hdr + idx
LDR   r0, [heap_base, r0, LSL #2]
@ store r0 into rd
```

`heap_base` is the base address of the arena memory. It could be held in a
dedicated location (e.g. `[r10, #-12]`) or computed from `r12 - hp_offset`.
Alternatively the heap address can be a direct byte pointer if we store
`heap_base + addr*4` in values instead of raw word indices — a possible
simplification for the AOT backend.

### 5.3. GLOBAL — load from globals array

**VM**: `regs[rd] = globals[idx]`

```arm
@ globals array at a known address, e.g. [r10, #-16] holds &globals
LDR   r1, [r10, #-16]          @ r1 = &globals[0]
LDR   r0, [r1, #(idx * 4)]
@ store r0 into rd
```

### 5.4. FUNCTION — materialize a code table index as a function value

**VM**: `regs[rd] = Value::function(code_ptr)` = `TYP_FUNC | (code_idx << 16)`

```arm
@ code_idx is a compile-time constant
MOVW  r0, #TYP_FUNC            @ 0x05
MOVT  r0, #code_idx            @ pack into upper 16 bits
@ store r0 into rd
```

This is a single 32-bit constant load (MOVW+MOVT) with no memory access.

### 5.5. CLOSURE — allocate a closure with captured values

**VM**: alloc `2 + ncap` words, write gc_header + closure_header + captures.

```arm
@ 1. Bump-allocate (size = 2 + ncap)
ADD   r0, r12, #((2 + ncap) * 4)
LDR   r1, [r10, #-4]           @ heap limit
CMP   r0, r1
BHI   .gc_slow_path
MOV   r1, r12                  @ r1 = object address
MOV   r12, r0                  @ bump

@ 2. Write GC header:  gc_header(2 + ncap)
MOV   r0, #(TYP_GC | ((2 + ncap) << 8))
STR   r0, [r1, #0]

@ 3. Write closure header: closure_header(ncap, code_idx)
MOVW  r0, #(TYP_HDR | (ncap << 8))
MOVT  r0, #code_idx
STR   r0, [r1, #4]

@ 4. Write captures
STR   cap0_reg, [r1, #8]       @ capture 0
STR   cap1_reg, [r1, #12]      @ capture 1
...

@ 5. Build closure value: Value::closure(addr)
@    addr = word index of r1 in heap
@    TYP_CLOS = 0, so value = (word_idx << 16)
@ compute word index from r1...
@ store into rd
```

### 5.6. PACK — construct a tagged value

**VM**: `regs[rd] = Value::ctor(tag, addr)` with fields on heap.

#### Nullary (arity == 0):

```arm
@ Value::ctor(tag, NULL) = TYP_CTOR | (tag << 8) | (0xFFFF << 16)
MOVW  r0, #(TYP_CTOR | (tag << 8))
MOVT  r0, #0xFFFF
@ store r0 into rd
```

No heap allocation. Single constant load.

#### Non-nullary (arity > 0):

```arm
@ 1. Bump-allocate (size = 1 + arity)
ADD   r0, r12, #((1 + arity) * 4)
LDR   r1, [r10, #-4]
CMP   r0, r1
BHI   .gc_slow_path
MOV   r1, r12
MOV   r12, r0

@ 2. Write GC header
MOV   r0, #(TYP_GC | ((1 + arity) << 8))
STR   r0, [r1, #0]

@ 3. Write fields
STR   field0_reg, [r1, #4]
STR   field1_reg, [r1, #8]
...

@ 4. Build ctor value: Value::ctor(tag, word_addr)
@    TYP_CTOR | (tag << 8) | (word_addr << 16)
@ store into rd
```

### 5.7. FIELD — project a field from a constructor

**VM**: `regs[rd] = heap[ctor.ctor_addr() + 1 + idx]`

```arm
@ rs contains a ctor value
LSR   r0, rs, #16              @ r0 = heap word address
ADD   r0, r0, #(1 + idx)       @ skip gc_header, offset to field
LDR   r0, [heap_base, r0, LSL #2]
@ store r0 into rd
```

### 5.8. UNPACK — destructure a constructor into consecutive registers

**VM**: for `i in 0..arity`: `regs[rd + i] = heap[ctor.ctor_addr() + 1 + i]`

```arm
@ rs contains a ctor value, arity known at compile time
LSR   r0, rs, #16              @ heap word address
ADD   r0, heap_base, r0, LSL #2  @ byte pointer to object

@ Load fields into consecutive destination registers/slots
LDR   r1, [r0, #4]             @ field 0 → rd+0
STR   r1, [r10, #offset_rd0]
LDR   r1, [r0, #8]             @ field 1 → rd+1
STR   r1, [r10, #offset_rd1]
...
```

If the destination registers happen to be pinned ARM registers, the STR becomes
a MOV.

### 5.9. MATCH — jump table on constructor tag

**VM**: extract `ctor_tag`, subtract `base`, index into offset table, jump.

ARM Thumb-2 has `TBB` (byte table branch) and `TBH` (halfword table branch)
instructions that map directly:

```arm
@ rs contains a ctor value
LSR   r0, rs, #8
AND   r0, r0, #0xFF            @ r0 = ctor_tag
SUB   r0, r0, #base
CMP   r0, #n_cases
BHS   .match_fail               @ unsigned >= means out of range

@ TBH: each entry is a halfword offset from the TBH itself
TBH   [PC, r0, LSL #1]
.Ltable:
    .hword (.Lcase0 - .Ltable) / 2
    .hword (.Lcase1 - .Ltable) / 2
    ...

.Lcase0:
    @ UNPACK if arity > 0, then case body
.Lcase1:
    ...

.match_fail:
    @ trap / call error handler
```

Each case arm is a straight-line block emitted right after the table.
The UNPACK for each case (if arity > 0) is inlined at the start of the case
block.

### 5.10. ENCORE — tail call

**VM**: set SELF = fun, CONT = cont, resolve code pointer, jump.

This is the core control flow operation. Because all calls in CPS are tail calls,
there is no stack frame to save or restore.

```arm
@ rf = function register, rk = continuation register
@ 1. Load fun and cont values
MOV   r4, rf_val               @ SELF = fun
MOV   r5, rk_val               @ CONT = cont

@ Arguments A1..An have already been staged by MOV instructions
@ emitted before ENCORE (same as bytecode emitter does today).

@ 2. Resolve code pointer
AND   r0, r4, #0xFF            @ type tag
CMP   r0, #5                   @ TYP_FUNC?
BNE   .closure_path

@ Function path: code_idx = payload
LSR   r0, r4, #16
LDR   r0, [r11, r0, LSL #2]   @ r0 = code_table[idx]
BX    r0

.closure_path:
@ Closure path: read header from heap
LSR   r0, r4, #16              @ heap word address
LDR   r0, [heap_base, r0, LSL #2 + 4]  @ closure header at slot 1
LSR   r0, r0, #16              @ code table idx
LDR   r0, [r11, r0, LSL #2]
BX    r0
```

The function/closure dispatch could be branchless using conditional execution
(IT blocks in Thumb-2), but the branch predictor will handle this well since
most call sites are monomorphic.

### 5.11. FIN — halt / return to caller

**VM**: `return read_reg(rs)`

FIN terminates the execution loop and returns to the Rust/C caller.
In AOT, each top-level entry point is called from Rust via a standard ARM
function call. FIN returns to the caller via `BX LR`:

```arm
@ Move result into r0 (ARM calling convention return register)
MOV   r0, rs_val
@ Restore callee-saved registers (r4-r9, r10-r11 if saved at entry)
POP   {r4-r11, PC}             @ return to caller
```

The entry point prolog (not part of FIN, but its counterpart) saves callee-saved
registers and sets up the runtime pointers:

```arm
_encore_entry:
    PUSH  {r4-r11, LR}
    @ Load runtime pointers from context struct passed in r0
    MOV   r10, r0               @ shadow register file
    LDR   r11, [r0, #-20]       @ code table base
    LDR   r12, [r0, #-8]        @ heap pointer
    @ Set up SELF, CONT, A1 from caller-provided arguments
    ...
    @ Jump to function body
    BX    ...
```

### 5.12. INT / INT_0 / INT_1 / INT_2 — integer constants

**VM**: `regs[rd] = Value::int(n)` = `TYP_INT | ((n & 0x00FFFFFF) << 8)`

```arm
@ INT_0: Value::int(0) = 0x04  (just TYP_INT, no payload)
MOV   r0, #4

@ INT_1: Value::int(1) = 0x104
MOV   r0, #0x104

@ INT_2: Value::int(2) = 0x204
MOVW  r0, #0x204

@ General INT(n):
MOVW  r0, #lower16
MOVT  r0, #upper16             @ if needed
@ store r0 into rd
```

All integer constants are compile-time known and encoded as immediates.

### 5.13. INT_ADD — integer addition

**VM**: extract int values, add, repack.

```arm
@ ra, rb are source registers (ARM regs or loaded from shadow file)
ASR   r0, ra_val, #8           @ a = int_value(ra) — sign-extending shift
ASR   r1, rb_val, #8           @ b = int_value(rb)
ADD   r0, r0, r1               @ a + b
LSL   r0, r0, #8               @ shift back to payload position
ORR   r0, r0, #TYP_INT         @ repack: [result << 8 | 4]
@ store r0 into rd
```

**Optimization**: since `Value::int(n) = (n << 8) | 4`, addition can be done
directly on the packed representation:

```arm
@ a_packed = (a << 8) | 4
@ b_packed = (b << 8) | 4
@ a_packed + b_packed = ((a+b) << 8) | 8
@ We need ((a+b) << 8) | 4, so subtract 4:
ADD   r0, ra_val, rb_val
SUB   r0, r0, #TYP_INT         @ correct the doubled type tag
@ store r0 into rd
```

This reduces integer addition to **2 instructions** (ADD + SUB).

### 5.14. INT_SUB — integer subtraction

Same tagged-value trick:

```arm
@ a_packed - b_packed = ((a-b) << 8) | 0
@ Need ((a-b) << 8) | 4, so add 4:
SUB   r0, ra_val, rb_val
ADD   r0, r0, #TYP_INT
@ store r0 into rd
```

Also **2 instructions**.

### 5.15. INT_MUL — integer multiplication

Multiplication doesn't have the same shortcut (the tags interfere):

```arm
ASR   r0, ra_val, #8           @ extract int a
ASR   r1, rb_val, #8           @ extract int b
MUL   r0, r0, r1
LSL   r0, r0, #8
ORR   r0, r0, #TYP_INT
@ store r0 into rd
```

**4 instructions**. Could also be done as:

```arm
ASR   r0, ra_val, #8
SMULL r0, r1, r0, rb_val       @ r0 = (a * b_packed) low 32 bits
BIC   r0, r0, #0xFF            @ clear the type tag bits that leaked in
ORR   r0, r0, #TYP_INT
```

### 5.16. INT_EQ — integer equality

**VM**: produces `Value::ctor(tag, HeapAddress::NULL)` where tag = 1 if equal,
0 if not. This is a nullary constructor — no heap allocation.

```arm
@ Since both values are packed the same way, we can compare directly
@ (the type tag is the same for both ints, so it cancels out)
CMP   ra_val, rb_val
ITE   EQ
MOVWEQ  r0, #0x0109            @ ctor(1, NULL) low bits: TYP_CTOR|(1<<8) = 0x0109.. wait
@ Let me compute:
@ Value::ctor(1, NULL) = TYP_CTOR | (1 << 8) | (0xFFFF << 16)
@                      = 0xFFFF_0101
@ Value::ctor(0, NULL) = TYP_CTOR | (0 << 8) | (0xFFFF << 16)
@                      = 0xFFFF_0001
```

Simpler approach with pre-loaded constants:

```arm
@ Pre-compute the two possible results
@ TRUE  = 0xFFFF_0101 = Value::ctor(1, NULL)
@ FALSE = 0xFFFF_0001 = Value::ctor(0, NULL)
CMP   ra_val, rb_val
ITE   EQ
LDREQ r0, =0xFFFF0101
LDRNE r0, =0xFFFF0001
@ store r0 into rd
```

Or, since the only difference is bit 8:

```arm
LDR   r0, =0xFFFF0001          @ start with FALSE
CMP   ra_val, rb_val
IT    EQ
ORREQ r0, r0, #0x100           @ set tag bit → TRUE
@ store r0 into rd
```

**3 instructions** (plus the constant load).

### 5.17. INT_LT — integer less-than

Same pattern as INT_EQ but with signed comparison:

```arm
@ Packed ints preserve order under signed comparison
@ because the tag is in the low bits (same for both)
@ and the payload is in the upper bits, sign-extended.
CMP   ra_val, rb_val
LDR   r0, =0xFFFF0001          @ FALSE
IT    LT
ORRLT r0, r0, #0x100           @ TRUE
@ store r0 into rd
```

**3 instructions**. Note: signed comparison on packed values works correctly
because both values have the same type tag (TYP_INT = 4) in the low byte, so
the comparison is determined entirely by the upper 24 bits, which are the
sign-extended integer payload.

### 5.18. EXTERN — foreign function call

**VM**: `regs[rd] = extern_fns[slot](read_reg(ra))`

Extern functions have signature `fn(u32) -> u32` (one Value in, one Value out).
The AOT emitter wraps this as a standard ARM function call:

```arm
@ Load extern function pointer
LDR   r1, [r10, #-24]          @ extern_table base
LDR   r1, [r1, #(slot * 4)]    @ fn ptr for this slot
@ Set up argument
MOV   r0, ra_val               @ argument in r0 (ARM calling convention)
@ Save volatile state
PUSH  {r2-r3, r12}             @ save A1, A2, HP
BLX   r1                       @ call extern
POP   {r2-r3, r12}             @ restore
@ r0 = result
@ store r0 into rd
```

The push/pop is needed because r2, r3, r12 are caller-saved in the ARM AAPCS.
Callee-saved registers (r4–r11) survive the call automatically.

Note: if the extern function can allocate or trigger GC, the full register
flush (as in the GC safepoint) would be needed before the call.

## 6. Runtime Data Layout

The shadow register file and associated metadata form a small runtime context
block:

```
Offset from r10    Contents
─────────────────────────────
  -24              &extern_table
  -20              &code_table (also in r11)
  -16              &globals
  -12              heap_base (arena start address)
   -8              heap_pointer (synced with r12)
   -4              heap_limit
    0              regs[0]  — SELF
    4              regs[1]  — CONT
    8              regs[2]  — A1
   12              regs[3]  — A2
   ...
  124              regs[31] — X22
```

This layout allows the GC and extern calls to access all runtime state through
a single pointer (r10).

## 7. Summary

| Aspect                | Strategy                                               |
|-----------------------|--------------------------------------------------------|
| Value repr            | Unchanged 32-bit packed values                         |
| Code pointers         | u16 index into code table; one extra LDR per call      |
| Hot registers         | 6 pinned (SELF, CONT, A1, A2, X01–X04)                |
| Cold registers        | Shadow register file in memory via r10                 |
| Heap allocation       | Bump pointer in r12; slow path calls GC                |
| GC roots              | Flush pinned regs to shadow file at safepoints         |
| GC algorithm          | Existing mark-compact, called from generated code      |
| Arithmetic            | 2 instructions for ADD/SUB, 4 for MUL, 3 for EQ/LT    |
| Pattern match         | TBH jump table                                         |
| Tail call (ENCORE)    | Resolve via code table, BX — no stack growth           |
| Extern calls          | Standard ARM ABI call with register save/restore       |
