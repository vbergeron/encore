# Fleche

Fleche is the surface language for the Encore system. It provides a direct-style functional syntax that is compiled through CPS transformation into bytecode for the Encore VM.

The Fleche frontend lives in the `encore_fleche` crate (lexer, parser). The IR types and compiler passes live in `encore_compiler`.

## Syntax

### Comments

Line comments start with `#` and run to end of line:

```
# This is a comment
define main as 42  # inline comment
```

### Identifiers

- **Lowercase** (`x`, `my_var`, `_tmp`): variables, function names, binders.
- **Uppercase** (`Zero`, `Succ`, `Pair`): data constructors.

Casing is significant — the parser uses it to distinguish variables from constructors.

### Data declarations

```
data Zero | Succ(n)
```

Declares constructors with their arities. The constructors `False` (tag 0) and `True` (tag 1) are pre-registered; user-declared constructors are assigned tags sequentially starting from `2`. If a `data` declaration re-declares `False` or `True`, the existing registration is kept. An optional leading pipe is allowed for multiline style:

```
data
  | Zero
  | Succ(n)
```

Nullary constructors omit parentheses. Fields in the declaration are named but only their count (arity) matters.

Multiple `data` declarations are allowed; tags continue from the previous declaration's last tag:

```
data Zero | Succ(n)    # tags 2, 3
data Nil | Cons(h, t)  # tags 4, 5
```

### Definitions

```
define main as <expr>
```

A module is a sequence of `data` declarations followed by `define` statements. Each define introduces a global binding.

Foreign functions are declared with `define extern`:

```
define extern my_print 0
```

This binds `my_print` as a global that calls extern slot `0` at runtime.

### Expressions

#### Integer literals

```
42
0
```

Non-negative decimal integers, represented as signed 24-bit values at runtime.

#### String literals

```
"hello"
```

String literals produce `Bytes` values. Each character maps to one byte.

#### Variables

```
x
```

#### Lambda

```
x -> body
```

Single-argument function. Currying is expressed by nesting:

```
x -> y -> x
```

#### Application

```
f x
f x y         # left-associative: (f x) y
f (g x)       # parentheses for nested application
```

Juxtaposition of atoms, left-associative.

#### Let

```
let x = expr1 in expr2
```

#### Recursive let

```
let rec f x = body in rest
```

Binds `f` as a recursive function with parameter `x` in both `body` and `rest`.

#### Constructor application

```
Zero            # nullary, no parens
Succ(n)         # unary
Pair(a, b)      # binary
```

The constructor must be declared in a `data` block and the number of arguments must match the declared arity.

#### Field access

```
field 0 of expr
field 1 of Pair(a, b)
```

Zero-indexed projection into a constructor's fields.

#### Match

```
match expr
  case Zero -> e1
  case Succ(pred) -> e2
end
```

Cases must cover a **contiguous range** of tags. The order of `case` branches does not matter; they are sorted by tag internally. Binders in parentheses are bound to the constructor's fields positionally.

#### Builtin operations

```
builtin add x y
builtin sub 10 3
builtin mul a b
builtin eq x y
builtin lt x y
builtin int_byte x
builtin bytes_len s
builtin bytes_get s i
builtin bytes_concat a b
builtin bytes_slice s i n
builtin bytes_eq a b
```

Primitive operations with atom arguments only. Integer arithmetic builtins (`add`, `sub`, `mul`) return integers. Comparison builtins (`eq`, `lt`, `bytes_eq`) return nullary constructors: tag `1` for true, tag `0` for false. `int_byte` converts an integer 0–255 to a single-byte string. Byte string builtins operate on `Bytes` values:

```
data False | True

define main as
  let r = builtin lt 3 5 in
  match r
    case False -> 0
    case True -> 1
  end
```

## Compiler pipeline

The frontend (`encore_fleche`) parses source text into a `ds::Module`. The backend (`encore_compiler`) transforms it through several IR layers into bytecode:

```
Source text
    │  encore_fleche::parse
    ▼
ds::Module       direct-style AST with named binders
    │  ds_uncurry::resolve_module
    ▼
ds::Module       uncurried (multi-arg lambdas and saturated applications)
    │  dsi_resolve::resolve_module
    ▼
dsi::Module      de Bruijn-indexed AST (nameless, capture-safe)
    │  cps_transform::transform_module
    ▼
cps::Module      continuation-passing style
    │  cps_optimize::optimize_module
    ▼
cps::Module      optimized CPS (shrinking + rewrite passes)
    │  asm_resolve::resolve_module
    ▼
asm::Module      resolved locations (registers, no names)
    │  asm_emit::Emitter::emit_module
    ▼
Vec<u8>          ENCR binary for encore_vm
```

### IR layers

All IR types are defined in `encore_compiler::ir`.

**`ds` (direct style)** — what the parser produces. Nested expressions, string-named binders. Variants: `Var`, `Lambda`, `Apply`, `Let`, `Letrec`, `Ctor`, `Field`, `Match`, `Int`, `Bytes`, `Prim`, `Extern`.

**`dsi` (direct style, indexed)** — de Bruijn-indexed version of `ds`. Variables are `Var(Index)` instead of `Var(String)`, lambdas are `Lam(body)` with implicit binders, `Let`/`Letrec` are nameless, and match cases carry an `arity` rather than a list of binder names. This representation makes the CPS transform capture-safe without name-tracking.

**`cps` (continuation-passing style)** — every subexpression is named by a `Let` binding. Functions (`Fun`) take multiple `args` and a `cont` as separate parameters. `Encore(f, args, k)` enters a closure with arguments and a continuation — this single form handles both function calls and continuation resumption. To resume a continuation, `k` is a `NullCont` sentinel (a dead value that will never be invoked). `Fin(name)` halts with a result. Continuation values (`Cont`) are multi-parameter lambdas used for return points. Values (`Val`) include `Var`, `Cont`, `NullCont`, `Ctor`, `Field`, `Int`, `Bytes`, `Prim`, `Extern` — all operating on names, not nested expressions.

**`asm` (assembly)** — names erased, replaced by `Reg` (`u8`) with fixed assignments: `SELF=0`, `CONT=1`, `A1=2`..`A8=9`, locals from `X01=10` upward, `NULL=0xFF`. Values are `Reg(Reg)`, `Capture(u8)`, `Global(u8)`, etc. Functions (`Fun`) and continuations (`ContLam`) each carry an explicit `captures: Vec<Reg>`. Control flow uses `Encore(fun_reg, arg_regs, cont_reg)` — continuation resumption passes `NULL` as the dead continuation. Ready for direct bytecode emission.

**`prim`** — the `PrimOp` enum shared across all IR layers.

### Passes

All passes are in `encore_compiler::pass`.

**DS uncurry** (`ds_uncurry`) — flattens nested single-argument lambdas into multi-argument lambdas where safe. Resolves curried applications to saturated calls when the callee's arity is known. Runs on `ds::Module` before name resolution.

**DSI resolve** (`dsi_resolve`) — converts `ds::Module` into `dsi::Module` by replacing named binders with de Bruijn indices. This makes the subsequent CPS transform capture-safe without name-tracking.

**CPS transform** (`cps_transform`) — converts `dsi::Expr` into `cps::Expr` using meta-continuations (Rust closures). Each dsi-level function becomes a cps-level `Fun` that takes `args` and a `cont` as separate parameters. Applications become `Encore(f, args, k)` calls. The transform ensures all calls are in tail position.

**CPS optimizer** (`cps_optimize`) — iterates shrinking reductions (dead code, copy propagation, constant folding, beta contraction, eta reduction) to a fixed point, interleaved with growth-enabling passes (inlining, hoisting, CSE, contification). Configurable per-pass via `OptimizeConfig`. See [OPTIMIZER.md](OPTIMIZER.md).

**ASM resolve** (`asm_resolve`) — performs closure conversion and name resolution. Computes free variables of each function and continuation lambda, determines which are captured vs. global, and assigns register locations (`SELF`, `CONT`, `A1`–`A8`, `X01`+, `NULL`). Recursive bindings (`Letrec`) get `SELF` access.

**ASM emit** (`asm_emit`) — walks the `asm` tree and outputs VM opcodes. Zero-capture closures use the `FUNCTION` opcode (no heap allocation); closures with captures use `CLOSURE`. Bodies are emitted via a deferred patching mechanism: a placeholder code pointer is filled in once the body is emitted after the current top-level define. The emitter also tracks constructor arities for the binary's arity table.

## Primitive operations

Defined in `ir::prim`, shared across all IR layers:

| `PrimOp` | Fleche syntax | VM opcode |
|----------|--------------|-----------|
| `Int(Add)` | `builtin add` | `INT_ADD` |
| `Int(Sub)` | `builtin sub` | `INT_SUB` |
| `Int(Mul)` | `builtin mul` | `INT_MUL` |
| `Int(Eq)` | `builtin eq` | `INT_EQ` |
| `Int(Lt)` | `builtin lt` | `INT_LT` |
| `Int(Byte)` | `builtin int_byte` | `INT_BYTE` |
| `Bytes(Len)` | `builtin bytes_len` | `BYTES_LEN` |
| `Bytes(Get)` | `builtin bytes_get` | `BYTES_GET` |
| `Bytes(Concat)` | `builtin bytes_concat` | `BYTES_CONCAT` |
| `Bytes(Slice)` | `builtin bytes_slice` | `BYTES_SLICE` |
| `Bytes(Eq)` | `builtin bytes_eq` | `BYTES_EQ` |
