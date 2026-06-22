# Encore!

Encore is a lightweight bytecode VM and CPS-transforming compiler designed to run **Rocq-extracted functional programs on resource-constrained targets**. The runtime is a `#![no_std]` Rust crate that fits in firmware; the compiler pipeline takes Rocq-extracted Scheme as its primary input and produces compact bytecode.

```
Rocq proof / program
    │  Extraction (Scheme)
    ▼
extracted .scm
    │  encore compile scheme
    ▼
  .encr bytecode
    │  encore_vm  (#![no_std])
    ▼
  Value
```

## Why Encore?

The name comes from the VM's single calling opcode: `ENCORE`. Every function call sets the callee and continuation registers and jumps without returning; there is no call stack. In French, *encore* means *again*, *still*, *more*. It fits.

Rocq's extraction mechanism produces correct-by-construction Scheme code, but running it anywhere below a full Lisp runtime has historically meant a large porting effort. Encore fills that gap: a bytecode interpreter with a built-in GC that compiles to a `no_std` Rust crate and links into firmware with a fixed heap budget.

## Crates

### `encore_vm`

The VM itself. Requires only a fixed arena; `#![no_std]`, brings its own GC.

- Packed 32-bit values: closures, constructors, integers, byte strings
- 256-register file, bump-allocation heap arena
- Mark-compact garbage collector
- Single calling convention: `ENCORE` opcode, set callee and continuation, jump without returning

See [VM.md](VM.md) for the value encoding, opcode table, and binary format. See [AOT.md](AOT.md) for the native/ahead-of-time compilation design.

### `encore_scheme`

Scheme/S-expression frontend for Rocq-extracted `.scm` files. Parses S-expressions, desugars `lambda`, `match`, `if`, `letrec`, and other special forms, then lowers to the same intermediate representation used by all compiler passes. This is the primary input path.

See [SCHEME.md](SCHEME.md) for the frontend reference.

### `encore_compiler`

The compiler backend. Owns all IR types and transformation passes:

- **DS uncurry**: flattens curried lambdas into multi-argument functions
- **DSI resolve**: named binders to de Bruijn indices
- **CPS transform**: converts the indexed AST into continuation-passing style
- **CPS optimizer**: shrinking reductions and growth-enabling passes (inlining, hoisting, CSE, contification)
- **ASM resolve**: closure conversion, register assignment
- **ASM peephole**: sinks redundant `MOV` instructions
- **ASM emit**: serializes to the ENCR binary format consumed by `encore_vm`

See [OPTIMIZER.md](OPTIMIZER.md) for the optimization passes.

### `encore`

Command-line interface. Useful for development and inspection on a host machine before deploying to a target:

- **`encore compile scheme`**: compile a Rocq-extracted `.scm` file to `.encr` bytecode
- **`encore run`**: load and execute a compiled `.encr` program on the host
- **`encore disasm`**: disassemble a binary (plain listing or interactive TUI)
- **`encore compile fleche`**: compile a Fleche source file (see below)

### `encore_disasm`

Bytecode disassembler and inspector. Decodes ENCR binaries into a human-readable instruction listing with automatic function labels. Includes an interactive TUI (ratatui/crossterm) for browsing.

### Dependency graph

```
encore ──► encore_scheme ──► encore_compiler ──► encore_vm
  │
  ├──► encore_fleche ──► encore_compiler
  │
  └──► encore_disasm ──► encore_vm
```

## Quick start

### Compile a Rocq-extracted Scheme file

```bash
encore compile scheme extracted.scm --out program.encr
```

### Run on host

```bash
encore run program.encr
encore run program.encr --entry 1          # run the second define (0-based)
encore run program.encr --heap-size 131072  # 128 K words of heap
```

### Inspect the bytecode

```bash
encore disasm program.encr                 # plain text listing
encore disasm program.encr --interactive   # ratatui TUI browser
```

### Optimizer flags

Every CPS optimization pass can be toggled individually. All default to `on`.

```bash
encore compile scheme extracted.scm --cps-optimize=off            # disable entirely
encore compile scheme extracted.scm --cps-optimize-rewrite-inlining=off
encore compile scheme extracted.scm --cps-optimize-fuel 200 --cps-optimize-inline-threshold 10
```

| Flag | Default | Description |
|------|---------|-------------|
| `--cps-optimize` | `on` | Enable/disable the optimizer entirely |
| `--cps-optimize-fuel` | `100` | Max optimizer iterations |
| `--cps-optimize-inline-threshold` | `8` | Max body size for inlining |
| `--cps-optimize-simplify-dead-code` | `on` | Dead code elimination |
| `--cps-optimize-simplify-copy-propagation` | `on` | Copy propagation |
| `--cps-optimize-simplify-constant-fold` | `on` | Constant folding |
| `--cps-optimize-simplify-beta-contraction` | `on` | Beta contraction |
| `--cps-optimize-simplify-eta-reduction` | `on` | Eta reduction |
| `--cps-optimize-rewrite-inlining` | `on` | Function inlining |
| `--cps-optimize-rewrite-hoisting` | `on` | Loop-invariant hoisting |
| `--cps-optimize-rewrite-cse` | `on` | Common subexpression elimination |
| `--cps-optimize-rewrite-contification` | `on` | Contification |

## Fleche

[Fleche](FLECHE.md) is a small functional language included in the repository as a test vehicle for the VM and compiler pipeline. It is not the intended production input (Rocq extraction is), but it is useful for writing targeted unit tests and validating new compiler passes without a full Rocq proof cycle.

```
data Zero | Succ(n)

let rec countdown n =
  match n
  | Zero -> 0
  | Succ(pred) -> builtin add 1 (countdown pred)
  end

let main = countdown Succ(Succ(Succ(Zero)))
```

```bash
encore compile fleche hello.fleche --out hello.encr
encore run hello.encr
```

## Building and testing

```bash
cargo build
cargo test
```
