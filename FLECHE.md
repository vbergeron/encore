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

Declares constructors with their arities. Tags are assigned sequentially starting from `0` in declaration order. An optional leading pipe is allowed for multiline style:

```
data
  | Zero
  | Succ(n)
```

Nullary constructors omit parentheses. Fields in the declaration are named but only their count (arity) matters.

Multiple `data` declarations are allowed; tags continue from the previous declaration's last tag:

```
data Zero | Succ(n)    # tags 0, 1
data True | False      # tags 2, 3
```

### Definitions

```
define main as <expr>
```

A module is a sequence of `data` declarations followed by `define` statements. Each define introduces a global binding.

### Expressions

#### Integer literals

```
42
0
```

Non-negative decimal integers, represented as signed 24-bit values at runtime.

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
```

Primitive integer operations with exactly two atom arguments. Arithmetic builtins (`add`, `sub`, `mul`) return integers. Comparison builtins (`eq`, `lt`) return nullary constructors: tag `1` for true, tag `0` for false — suitable for matching:

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
    │  dsi_resolve::resolve_module
    ▼
dsi::Module      de Bruijn-indexed AST (nameless, capture-safe)
    │  cps_transform::transform_module
    ▼
cps::Module      continuation-passing style, ANF
    │  cps_optimize::optimize_module
    ▼
cps::Module      optimized CPS (shrinking + rewrite passes)
    │  asm_resolve::resolve_module
    ▼
asm::Module      resolved locations (no names)
    │  asm_emit::Emitter::emit_module
    ▼
Vec<u8>          ENCR binary for encore_vm
```

### IR layers

All IR types are defined in `encore_compiler::ir`.

**`ds` (direct style)** — what the parser produces. Nested expressions, string-named binders. Variants: `Var`, `Lam`, `App`, `Let`, `Letrec`, `Ctor`, `Field`, `Match`, `Int`, `Prim`.

**`dsi` (direct style, indexed)** — de Bruijn-indexed version of `ds`. Variables are `Var(Index)` instead of `Var(String)`, lambdas are `Lam(body)` with implicit binders, `Let`/`Letrec` are nameless, and match cases carry an `arity` rather than a list of binder names. This representation makes the CPS transform capture-safe without name-tracking.

**`cps` (continuation-passing style)** — administrative normal form with a uniform calling convention. Every subexpression is named by a `Let` binding. Functions (`Fun`) take an `arg` and a `cont` as separate parameters. `Encore(f, arg, k)` enters a closure with an argument and a continuation — this single form handles both function calls and continuation resumption. To resume a continuation, `k` is a `NullCont` sentinel (a dead value that will never be invoked). `Fin(name)` halts with a result. Continuation values (`Cont`) are single-parameter lambdas used for return points. Values (`Val`) include `Var`, `Cont`, `NullCont`, `Ctor`, `Field`, `Int`, `Prim` — all operating on names, not nested expressions.

**`asm` (assembly)** — names erased, replaced by `Loc`: `Arg`, `Cont`, `NullCont`, `Local(i)`, `Capture(i)`, `Global(i)`, `SelfRef`. Functions (`Fun`) and continuations (`ContLam`) each carry an explicit `captures: Vec<Loc>`. Control flow uses `Encore(fun, arg, cont)` — continuation resumption passes `NullCont` as the dead continuation. Ready for direct bytecode emission.

**`prim`** — the `PrimOp` enum shared across all IR layers.

### Passes

All passes are in `encore_compiler::pass`.

**DSI resolve** (`dsi_resolve`) — converts `ds::Module` into `dsi::Module` by replacing named binders with de Bruijn indices. This makes the subsequent CPS transform capture-safe without name-tracking.

**CPS transform** (`cps_transform`) — converts `dsi::Expr` into `cps::Expr` using meta-continuations (Rust closures). Each dsi-level function becomes a cps-level `Fun` that takes an `arg` and a `cont` as two separate parameters. Applications become `Encore(f, arg, k)` calls. The transform ensures all calls are in tail position.

**CPS optimizer** (`cps_optimize`) — iterates shrinking reductions (dead code, copy propagation, constant folding, beta contraction, eta reduction) to a fixed point, interleaved with growth-enabling passes (inlining, hoisting, CSE, contification). Configurable per-pass via `OptimizeConfig`. See [OPTIMIZER.md](OPTIMIZER.md).

**ASM resolve** (`asm_resolve`) — performs closure conversion and name resolution. Computes free variables of each function and continuation lambda, determines which are captured vs. global, and assigns `Local`/`Capture`/`Global`/`Arg`/`Cont`/`SelfRef` locations. Recursive bindings (`Letrec`) get `SelfRef` access.

**ASM emit** (`asm_emit`) — walks the `asm` tree and outputs VM opcodes. Zero-capture closures use the `FUNCTION` opcode (no heap allocation); closures with captures use `CLOSURE`. Bodies are emitted via a deferred patching mechanism: a placeholder code pointer is filled in once the body is emitted after the current top-level define. The emitter also tracks constructor arities for the binary's arity table.

## Primitive operations

Defined in `ir::prim`, shared across all IR layers:

| `PrimOp` | Fleche syntax | VM opcode |
|----------|--------------|-----------|
| `Add` | `builtin add` | `INT_ADD` |
| `Sub` | `builtin sub` | `INT_SUB` |
| `Mul` | `builtin mul` | `INT_MUL` |
| `Eq` | `builtin eq` | `INT_EQ` |
| `Lt` | `builtin lt` | `INT_LT` |
