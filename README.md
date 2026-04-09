# Encore

A small functional language compiled through CPS transformation to a tail-call-only virtual machine.

## Architecture

```
Fleche source
    │  parse
    ▼
 ds::Module        direct-style AST (nested expressions, named binders)
    │  cps_transform
    ▼
cps::Module        continuation-passing style (ANF, explicit continuations)
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

### `encore_vm`

A `#![no_std]` bytecode interpreter with:

- Packed 32-bit values (closures, constructors, integers)
- Unified heap + stack arena
- Mark-compact garbage collector
- Tail-call-only execution via `ENCORE`

See [VM.md](VM.md) for details.

### `encore_compiler`

A compiler from the Fleche surface language down to VM bytecode:

- **Fleche frontend** — lexer and recursive-descent parser producing a direct-style AST
- **CPS transform** — converts nested expressions into continuation-passing style
- **Resolver** — closure conversion and name resolution to machine locations
- **Emitter** — generates VM bytecode and serializes the program binary

See [FLECHE.md](FLECHE.md) for details.

## Quick example

```
data Zero | Succ(n)

define main as
  fix countdown n =
    match n
      case Zero -> 0
      case Succ(pred) -> builtin add 1 (countdown pred)
    end
  in countdown Succ(Succ(Succ(Zero)))
```

## Building and testing

```bash
cargo test
```
