# CPS Optimizer

This document describes the optimization passes applied to the CPS intermediate representation, their ordering, and the references behind them.

The optimizer lives in `encore_compiler::pass::cps_optimize` and is configured via `OptimizeConfig`, which provides per-pass toggle flags and numeric tuning parameters. All passes default to enabled.

## Pass structure

The optimizer is organized into two categories:

- **Simplify** (`pass/simplify/`) — shrinking reductions that never increase code size, iterated to a fixed point.
- **Rewrite** (`pass/rewrite/`) — growth-enabling passes that may increase code size, each followed by a simplify round.

The outer loop interleaves rewrite passes with simplify rounds, controlled by a `fuel` counter that limits iterations.

## Shrinking reductions (simplify)

These are applied in sequence and looped until no changes occur.

### Dead code elimination (`simpl_01_dead_code`)

```
# let unused = Succ(x) in body   ──►   body
let unused = Succ(x) in body
```

`Let(x, val, body)` where `x` never appears in `body` drops the binding. Similarly for `Letrec`. Safe because all values in this IR are pure.

### Copy propagation (`simpl_02_copy_propagation`)

```
# let y = x in f y   ──►   f x
let y = x in f y
```

`Let(x, Var(y), body)` is eliminated by substituting `y` for `x`. The CPS transform produces these whenever a variable flows through a trivial continuation.

### Constant folding (`simpl_03_constant_fold`)

```
# let x = 3 in let y = 4 in builtin add x y   ──►   7
let x = 3 in let y = 4 in builtin add x y
```

Evaluates `Prim(op, [a, b])` where both operands are known `Int` values at compile time. Folds `Add`, `Sub`, `Mul` into `Int` results. Comparisons (`Eq`, `Lt`) are not folded because they produce constructors, not integers, and folding them would require knowing the tag convention at the CPS level.

### Beta contraction (`simpl_04_beta_contraction`)

```
# let k = x -> x in k arg   ──►   arg
let k = x -> x in k arg
```

If a `Let(x, Lambda(p, body), ...)` binds a lambda that is called exactly once, inline the lambda body at the call site, substituting the argument for `p`. This is the single most impactful optimization: the CPS transform generates a continuation for every application, and nearly all of them are called exactly once.

### Eta reduction (`simpl_05_eta_reduction`)

```
# let g = x -> f x in g arg   ──►   f arg
let g = x -> f x in g arg
```

A lambda `Lambda(x, App(f, x))` that just forwards to `f` (where `x != f`) is replaced by `Var(f)`. Collapses trivial wrapper continuations that the CPS transform sometimes produces.

## Growth-enabling passes (rewrite)

Each of these may increase code size. The simplify loop runs after each one to clean up newly exposed redexes.

### Inlining (`rewrite_01_inlining`)

```
# let double = x -> builtin add x x
# in double 3   ──►   builtin add 3 3
let double = x -> builtin add x x in double 3
```

Duplicates small function bodies at call sites. Uses an Appel-style heuristic: a function is inlined only if its body size (measured by AST node count) is below `inline_threshold` (default 20). Recursive functions (`Letrec`) are never inlined to avoid unbounded expansion.

### Hoisting (`rewrite_02_hoisting`) — stub

```
# let rec loop n =              let one = 1 in
#   let one = 1 in              let rec loop n =
#   builtin add n one    ──►      builtin add n one
```

Loop-invariant code motion. Moves closure allocations and computations out of self-recursive functions when they only depend on variables that don't change between iterations.

### Common subexpression elimination (`rewrite_03_cse`) — stub

```
# let a = field 0 of x in       let a = field 0 of x in
# let b = field 0 of x in       ... a ... a ...
# ... a ... b ...         ──►
```

Reuses a previously computed value instead of recomputing it. CPS names every intermediate value, which makes detection straightforward.

### Contification (`rewrite_04_contification`)

```
# let rec f x k = k r         let cont f x = r
# in ... f a k ...     ──►    in ... f a ...
```

Turns escaping functions into local continuations when escape analysis shows the function is only ever called with the same continuation. A contified function no longer needs to allocate a closure or pass/receive a continuation — it becomes a local jump. This is especially effective after inlining, where formerly separate call sites collapse and the continuation argument becomes uniform.

## Configuration

`OptimizeConfig` provides fine-grained control:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `fuel` | `usize` | `100` | Max outer-loop iterations |
| `inline_threshold` | `usize` | `20` | Max body size for inlining |
| `simplify_dead_code` | `bool` | `true` | Toggle dead code elimination |
| `simplify_copy_propagation` | `bool` | `true` | Toggle copy propagation |
| `simplify_constant_fold` | `bool` | `true` | Toggle constant folding |
| `simplify_beta_contraction` | `bool` | `true` | Toggle beta contraction |
| `simplify_eta_reduction` | `bool` | `true` | Toggle eta reduction |
| `rewrite_inlining` | `bool` | `true` | Toggle function inlining |
| `rewrite_hoisting` | `bool` | `true` | Toggle loop-invariant hoisting |
| `rewrite_cse` | `bool` | `true` | Toggle CSE |
| `rewrite_contification` | `bool` | `true` | Toggle contification |

These are exposed as CLI flags on `encore compile fleche` (e.g. `--cps-optimize-simplify-dead-code=off`).

## Pass ordering

```
repeat (up to fuel iterations):
  shrinking reductions to fixed point
  inlining       → shrinking reductions
  hoisting       → shrinking reductions
  CSE            → shrinking reductions
  contification  → shrinking reductions
until nothing changes

then:
  closure conversion (asm_resolve)
  bytecode emission (asm_emit)
```

The key invariant: after every pass that can grow or restructure code, re-stabilize with shrinking reductions before proceeding. This keeps the IR clean and ensures each subsequent pass sees the simplest possible input.

## Lowering passes

Applied once after the optimization loop, before bytecode emission.

### Closure conversion

Implemented in `pass/asm_resolve.rs`. Computes free variables of each function and continuation lambda, determines captures vs. globals, and assigns `Local`/`Capture`/`Global`/`Arg`/`Cont`/`SelfRef` locations. Should come after all optimizations so it sees the smallest possible free variable sets.

### Zero-env closure detection

After closure conversion, closures with no captures use a cheaper representation. The `FUNCTION` opcode packs the code address directly into the 32-bit value (in the addr field, with `ncap=0`), skipping both the heap allocation and the heap indirection at call time. `ENCORE` branches on `ncap` to decide whether to read the code pointer from the value or from the heap. This pairs naturally with hoisting, which is likely to produce zero-capture closures by moving bindings to outer scopes.

## References

1. **Appel, Andrew W.** *Compiling with Continuations*. Cambridge University Press, 1992. The canonical reference for CPS-based compilation. Covers beta-contraction, constant folding, eta-reduction, inlining, hoisting, closure conversion, and their cascading interaction.

2. **Kennedy, Andrew.** "Compiling with Continuations, Continued." *ICFP 2007*. Refines Appel's approach with a graph-based CPS representation using union-find for efficient eta-reduction.

3. **Cong, Youyou et al.** "Compiling with Continuations, or without? Whatever." *ICFP 2019*. Compares CPS-based and direct-style compilation, discusses administrative redex elimination and local continuation optimizations.

4. **Danvy, Olivier & Filinski, Andrzej.** "Representing Control: A Study of the CPS Transformation." *Mathematical Structures in Computer Science*, 1992. The higher-order CPS transform that avoids generating administrative redexes, using a callback function (meta-continuation) instead of a syntactic continuation.

5. **Appel, Andrew W.** "SSA is Functional Programming." *ACM SIGPLAN Notices*, 1998. + **Kelsey, Richard.** "A Correspondence between Continuation Passing Style and Static Single Assignment Form." *IR '95*, 1995. Establishes the formal equivalence between CPS and SSA, meaning SSA optimization literature applies directly to CPS IRs.

6. **Wingo, Andy.** "CPS Soup" and related posts on *wingolog.org*, 2014-2015. Practical account of how these passes are sequenced in the Guile Scheme compiler, including contification and its interaction with other passes.
