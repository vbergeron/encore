# Encore

A small functional language compiled through CPS transformation to a tail-call-only virtual machine.

The name comes from the VM's only calling convention: every function call is a tail call performed by the `ENCORE` opcode, which enters a closure, resets the stack, and never returns. There is no `CALL`/`RET` pair — continuations are passed explicitly as values. The compiler ensures this by CPS-transforming direct-style source code so that every application ends up in tail position, making `ENCORE` the one instruction needed to express all control flow.

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
