# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build
cargo build --release

# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p encore_compiler
cargo test -p encore_vm

# Run a single test by name
cargo test <test_name> -- --nocapture

# Run the CLI
cargo run --bin encore -- run <file.encr>
cargo run --bin encore -- compile fleche <file.fleche> --out <dir>
cargo run --bin encore -- disasm <file.encr> [--interactive]
```

## Architecture

Encore is a compiler + bytecode VM for **Fleche**, a functional language. Every function call is a tail call — there is no call stack. All calls go through the single `ENCORE` opcode, which sets `SELF`/`CONT` registers and jumps. Continuations are first-class values; the CPS transform makes them explicit.

### IR Pipeline

```
Fleche source
  → ds::Module       (direct-style AST, named binders)
  → ds::Module       (uncurried: multi-arg lambdas, saturated apps)   [ds_uncurry]
  → dsi::Module      (de Bruijn indexed, capture-safe)                [dsi_resolve]
  → cps::Module      (explicit continuations)                         [cps_transform]
  → cps::Module      (optimized)                                      [cps_optimize]
  → asm::Module      (registers, captures, globals — no names)        [asm_resolve]
  → bytecode         (ENCR binary)                                     [asm_emit]
```

All IR types and compiler passes live in `crates/encore_compiler/src/`. IR representations are in `ir/` subdirectory; passes are in `pass/`.

### Crate Overview

| Crate | Role |
|---|---|
| `encore` | CLI: `run`, `compile fleche/scheme`, `disasm` |
| `encore_compiler` | All IR types + transformation passes + `pipeline.rs` entry point |
| `encore_fleche` | Fleche lexer & parser → `ds::Module` |
| `encore_scheme` | S-expression parser → `ds::Module` (for Rocq-extracted code) |
| `encore_vm` | `#![no_std]` bytecode interpreter — `Vm`, `Value`, `Opcode`, GC |
| `encore_disasm` | Bytecode disassembler with ratatui TUI |
| `encore_derive` | Proc-macros: `ValueEncode` / `ValueDecode` for VM FFI |

### VM

Values are 32-bit packed words: `[payload:16 | meta:8 | typ:8]`. Types include `Closure`, `Constructor`, `Integer` (±8M range), `Function`, `Bytes`. There is a 256-register file (special: `SELF`, `CONT`, `A1–A8`, `NULL`). The heap uses bump allocation with a mark-compact GC.

Key opcodes: `ENCORE` (the only call), `FIN` (halt), `CLOSURE`/`FUNCTION`/`PACK` (allocation), `FIELD`/`UNPACK`/`MATCH`/`BRANCH` (destructuring), `MOV`/`CAPTURE`/`GLOBAL` (data movement).

### CPS Optimizer (`cps_optimize`)

Two interleaved pass families, iterated until fixed-point (default 100 fuel):
- **Simplify** (shrinking): dead code, copy propagation, constant folding, beta contraction, eta reduction
- **Rewrite** (growth-enabling): inlining, hoisting, CSE, contification

All passes are individually toggleable via CLI flags (e.g. `--cps-optimize-rewrite-inlining=off`). See `OPTIMIZER.md` for pass details.

## Reference Docs

- `FLECHE.md` — language syntax, keywords, data constructors
- `VM.md` — value encoding, opcodes, binary format
- `OPTIMIZER.md` — CPS pass descriptions and tuning
