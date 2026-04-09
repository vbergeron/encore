# Fleche

Fleche is the surface language for the Encore compiler. It provides a direct-style functional syntax that is compiled through CPS transformation into bytecode for the Encore VM.

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

#### Fix (recursive let)

```
fix f x = body in rest
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

```
Source text
    │  fleche::parse
    ▼
ds::Module       direct-style AST with named binders
    │  cps_transform::transform_module
    ▼
cps::Module      continuation-passing style, ANF
    │  resolver::resolve_module
    ▼
asm::Module      resolved locations (no names)
    │  Emitter::emit_module
    ▼
Vec<u8>          ENCR binary for encore_vm
```

### IR layers

**`ds` (direct style)** — what the parser produces. Nested expressions, string-named binders. Variants: `Var`, `Lam`, `App`, `Let`, `Letrec`, `Ctor`, `Field`, `Match`, `Int`, `Prim`.

**`cps` (continuation-passing style)** — administrative normal form. Every subexpression is named by a `Let` binding. Calls are `App(name, name)` (both operands are names). Functions receive a pair `(argument, continuation)` packed as `Ctor(255, [arg, k])`. `Fin(name)` halts with a result. Values (`Val`) include `Var`, `Lambda`, `Ctor`, `Field`, `Int`, `Prim` — all operating on names, not nested expressions.

**`asm` (assembly)** — names erased, replaced by `Loc`: `Arg`, `Local(i)`, `Capture(i)`, `Global(i)`, `SelfRef`. Lambdas carry an explicit `captures: Vec<Loc>`. Ready for direct bytecode emission.

### Passes

**CPS transform** — converts `ds::Expr` into `cps::Expr` using meta-continuations (Rust closures). Each ds-level function becomes a cps-level function that receives a `(arg, continuation)` pair via a reserved constructor tag `255`. The transform ensures all calls are in tail position.

**Resolver** — performs closure conversion and name resolution. Computes free variables of each lambda, determines which are captured vs. global, and assigns `Local`/`Capture`/`Global`/`Arg`/`SelfRef` locations. Recursive bindings (`Letrec`) get `SelfRef` access.

**Emitter** — walks the `asm` tree and outputs VM opcodes. Closure bodies are emitted via a deferred patching mechanism: a `CLOSURE` instruction writes a placeholder code pointer that is filled in once the body is emitted after the current top-level define. The emitter also tracks constructor arities for the binary's arity table.

## Primitive operations

Defined in `ir/prim.rs`, shared across all IR layers:

| `PrimOp` | Fleche syntax | VM opcode |
|----------|--------------|-----------|
| `Add` | `builtin add` | `INT_ADD` |
| `Sub` | `builtin sub` | `INT_SUB` |
| `Mul` | `builtin mul` | `INT_MUL` |
| `Eq` | `builtin eq` | `INT_EQ` |
| `Lt` | `builtin lt` | `INT_LT` |
