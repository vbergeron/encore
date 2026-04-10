# Encore

A small functional language compiled through CPS transformation to a tail-call-only virtual machine.

The name comes from the VM's only calling convention: every function call is a tail call performed by the `ENCORE` opcode, which enters a closure, resets the stack, and never returns. There is no `CALL`/`RET` pair — continuations are passed explicitly as values. The compiler ensures this by CPS-transforming direct-style source code so that every application ends up in tail position, making `ENCORE` the one instruction needed to express all control flow.

## Architecture

```
Fleche source
    │  encore_fleche::parse
    ▼
 ds::Module        direct-style AST (nested expressions, named binders)
    │  cps_transform
    ▼
cps::Module        continuation-passing style (ANF, explicit continuations)
    │  cps_optimize
    ▼
cps::Module        optimized CPS
    │  resolver
    ▼
asm::Module        resolved locations (locals, captures, globals — no names)
    │  emit
    ▼
  bytecode         binary format consumed by encore_vm
    │  run
    ▼
  Value            packed 32-bit runtime value
```

## Crates

### `encore`

Command-line interface that ties the frontend and backend together. Provides two subcommands:

- **`encore run`** — load and execute a compiled `.bin` program
- **`encore compile fleche`** — parse Fleche source and compile to bytecode

See [CLI usage](#cli-usage) below.

### `encore_fleche`

The Fleche language frontend — lexer and recursive-descent parser producing a direct-style AST (`ds::Module`). Depends on `encore_compiler` for the IR type definitions.

See [FLECHE.md](FLECHE.md) for the language reference.

### `encore_compiler`

The compiler backend. Owns all IR types (`ds`, `prim`, `cps`, `asm`) and all transformation passes:

- **CPS transform** — converts nested expressions into continuation-passing style
- **CPS optimizer** — shrinking reductions and growth-enabling passes (inlining, hoisting, CSE)
- **Resolver** — closure conversion and name resolution to machine locations
- **Emitter** — generates VM bytecode and serializes the program binary

See [FLECHE.md](FLECHE.md) for the compiler pipeline and [OPTIMIZER.md](OPTIMIZER.md) for the optimization passes.

### `encore_vm`

A `#![no_std]` bytecode interpreter with:

- Packed 32-bit values (closures, constructors, integers)
- Unified heap + stack arena
- Mark-compact garbage collector
- Tail-call-only execution via `ENCORE`

See [VM.md](VM.md) for details.

### Dependency graph

```
encore ──► encore_fleche ──► encore_compiler ──► encore_vm
  │                                │
  └────────────────────────────────┘
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

### Optimizer flags for `compile fleche`

Every CPS optimization pass can be toggled individually with `--cps-optimize-<pass>=on/off`. All default to `on`.

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

## Building and testing

```bash
cargo test
```
