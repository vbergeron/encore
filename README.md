# Encore

Encore is a lightweight bytecode virtual machine where every function call is a tail call. There is no call stack — continuations are first-class values and control flow is expressed entirely through the `ENCORE` opcode, which enters a closure, resets the evaluation stack, and never returns. A CPS-transforming compiler targeting the VM is included, along with [Fleche](FLECHE.md), a small functional language that serves as its frontend.

## Architecture

```
Fleche source
    │  encore_fleche::parse
    ▼
 ds::Module        direct-style AST (nested expressions, named binders)
    │  dsi_resolve
    ▼
dsi::Module        de Bruijn-indexed AST (nameless, capture-safe)
    │  cps_transform
    ▼
cps::Module        continuation-passing style (ANF, explicit continuations)
    │  cps_optimize
    ▼
cps::Module        optimized CPS
    │  asm_resolve
    ▼
asm::Module        resolved locations (locals, captures, globals — no names)
    │  asm_emit
    ▼
  bytecode         binary format consumed by encore_vm
    │  run
    ▼
  Value            packed 32-bit runtime value
```

## Crates

### `encore`

Command-line interface that ties the frontend and backend together. Provides three subcommands:

- **`encore run`** — load and execute a compiled `.bin` program
- **`encore compile fleche`** — parse Fleche source and compile to bytecode
- **`encore disasm`** — disassemble a compiled `.bin` program (plain or interactive TUI)

See [CLI usage](#cli-usage) below.

### `encore_fleche`

The Fleche language frontend — lexer and recursive-descent parser producing a direct-style AST (`ds::Module`). Depends on `encore_compiler` for the IR type definitions.

See [FLECHE.md](FLECHE.md) for the language reference.

### `encore_compiler`

The compiler backend. Owns all IR types (`ds`, `dsi`, `prim`, `cps`, `asm`) and all transformation passes:

- **DSI resolve** — converts named binders to de Bruijn indices for capture-safe CPS transform
- **CPS transform** — converts the indexed AST into continuation-passing style
- **CPS optimizer** — shrinking reductions and growth-enabling passes (inlining, hoisting, CSE, contification)
- **ASM resolve** — closure conversion and name resolution to machine locations
- **ASM emit** — generates VM bytecode and serializes the program binary

See [FLECHE.md](FLECHE.md) for the compiler pipeline and [OPTIMIZER.md](OPTIMIZER.md) for the optimization passes.

### `encore_vm`

A `#![no_std]` bytecode interpreter with:

- Packed 32-bit values (closures, constructors, integers)
- Unified heap + stack arena
- Mark-compact garbage collector
- Two-opcode calling convention: `ENCORE` (enter closure) and `RETURN` (resume continuation)

See [VM.md](VM.md) for details.

### `encore_disasm`

Bytecode disassembler and inspector. Decodes ENCR binaries into a human-readable instruction listing with automatic function labels. Includes an interactive TUI (ratatui/crossterm) for browsing.

### Dependency graph

```
encore ──► encore_fleche ──► encore_compiler ──► encore_vm
  │            │
  ├────────────┘
  ├──► encore_disasm ──► encore_vm
  └──► encore_compiler
```

## Quick example

```
data Zero | Succ(n)

define main as
  let rec countdown n =
    match n
      case Zero -> 0
      case Succ(pred) -> builtin add 1 (countdown pred)
    end
  in countdown Succ(Succ(Succ(Zero)))
```

## CLI usage

### Compile a Fleche program

```bash
encore compile fleche hello.fleche --out hello.bin
```

### Run a compiled binary

```bash
encore run hello.bin
```

### Options for `run`

```bash
encore run hello.bin --entry 1          # run the second define (0-based)
encore run hello.bin --heap-size 131072  # 128K words of heap
```

### Disassemble a binary

```bash
encore disasm hello.bin                 # plain text listing
encore disasm hello.bin --interactive   # ratatui TUI browser
```

### Optimizer flags for `compile fleche`

Every CPS optimization pass can be toggled individually with `--cps-optimize-<pass>=on/off`. All default to `on`. The optimizer as a whole can be disabled with `--cps-optimize=off`.

```bash
# Disable dead code elimination
encore compile fleche hello.fleche --cps-optimize-simplify-dead-code=off

# Disable all inlining
encore compile fleche hello.fleche --cps-optimize-rewrite-inlining=off

# Tune optimizer iterations and inline threshold
encore compile fleche hello.fleche --cps-optimize-fuel 200 --cps-optimize-inline-threshold 10
```

Available flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--cps-optimize` | `on` | Enable/disable the optimizer entirely |
| `--cps-optimize-fuel` | `100` | Max optimizer iterations |
| `--cps-optimize-inline-threshold` | `20` | Max body size for inlining |
| `--cps-optimize-simplify-dead-code` | `on` | Dead code elimination |
| `--cps-optimize-simplify-copy-propagation` | `on` | Copy propagation |
| `--cps-optimize-simplify-constant-fold` | `on` | Constant folding |
| `--cps-optimize-simplify-beta-contraction` | `on` | Beta contraction |
| `--cps-optimize-simplify-eta-reduction` | `on` | Eta reduction |
| `--cps-optimize-rewrite-inlining` | `on` | Function inlining |
| `--cps-optimize-rewrite-hoisting` | `on` | Loop-invariant hoisting |
| `--cps-optimize-rewrite-cse` | `on` | Common subexpression elimination |
| `--cps-optimize-rewrite-contification` | `on` | Contification (turn escaping functions into local continuations) |

## Building and testing

```bash
cargo test
```
