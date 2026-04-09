# CPS Optimizer

This document describes the optimization passes applied to the CPS intermediate representation, their ordering, and the references behind them.

## Current Implementation

The optimizer (`pass/cps_optimize.rs`) implements a census-based contraction pass iterated to a fixed point using structural equality on the CPS tree. Two contractions are currently implemented:

- **Dead code elimination** — `Let(x, val, body)` where `x` is unused in `body` is dropped (all vals are pure).
- **Copy propagation** — `Let(x, Var(y), body)` is eliminated by substituting `y` for `x` throughout `body`.

## Shrinking Reductions

These never increase code size and should be run to a fixed point before and after every growth-enabling pass.

### Beta contraction

If a `Let(x, Lambda(p, body), ...)` binds a lambda that is called exactly once, inline the lambda body at the call site, substituting the argument for `p`. This is the single most impactful optimization: the CPS transform generates a continuation for every application, and nearly all of them are called exactly once. Inlining them eliminates most administrative overhead.

### Constant folding

Evaluate `Let(x, Prim(Add, [a, b]), body)` where `a` and `b` are known `Int` values at compile time, producing `Let(x, Int(a+b), body)`. Extends to `Eq` and `Lt` producing known constructor tags.

### Dead code elimination (implemented)

`Let(x, val, body)` where `x` never appears in `body` drops the binding. Similarly for `Letrec`. Safe because all values in this IR are pure.

### Copy propagation (implemented)

`Let(x, Var(y), body)` is eliminated by substituting `y` for `x`. The CPS transform produces these whenever a variable flows through a trivial continuation.

### Eta reduction

A lambda `Lambda(x, App(f, x))` that just forwards to `f` can be replaced by `Var(f)`. Collapses trivial wrapper continuations that the CPS transform sometimes produces.

## Growth-Enabling Passes

Each of these may increase code size. Re-stabilize with shrinking reductions after each one.

### Inlining

Expand small known functions at call sites. Requires heuristics (body size, call count) to avoid code explosion. Because CPS makes all calls explicit, the compiler can inline without ambiguity about the call stack. Inlining exposes new beta-redexes and constant-folding opportunities.

### Hoisting

Loop-invariant code motion in CPS. Moves closure allocations and computations out of frequently-invoked continuations (self-recursive `Letrec` bodies) when they only close over variables that don't change between iterations. In the Encore VM, where every closure is a heap allocation, this directly reduces GC pressure. Requires detecting loops as self-tail-calling continuations (via SCC analysis on the call graph) and identifying which variables are loop-invariant vs. loop-varying.

### Common subexpression elimination

CPS names every intermediate value, which makes it straightforward to detect when two bindings compute the same thing. Comes last among the semantic optimizations because earlier passes rename and restructure heavily.

## Lowering Passes

Applied once after the optimization loop, before bytecode emission.

### Closure conversion

Already implemented in `pass/resolver.rs`. Computes free variables of each lambda, determines captures vs. globals, and assigns `Local`/`Capture`/`Global`/`Arg`/`SelfRef` locations. Should come after all optimizations so it sees the smallest possible free variable sets.

### Zero-env closure detection

After closure conversion, closures with no captures could use a cheaper representation. A dedicated `FUNCTION` opcode would skip environment setup, saving both the heap allocation and the indirection at call time. This pairs naturally with hoisting, which is likely to produce zero-capture closures by moving bindings to outer scopes.

## Pass Ordering

```
repeat:
  shrinking reductions to fixed point
  inlining
  shrinking reductions to fixed point
  hoisting
  shrinking reductions to fixed point
  CSE
until nothing changes

then:
  closure conversion
  zero-env closure detection
  bytecode emission
```

The key invariant: after every pass that can grow or restructure code, re-stabilize with shrinking reductions before proceeding. This keeps the IR clean and ensures each subsequent pass sees the simplest possible input.

## References

1. **Appel, Andrew W.** *Compiling with Continuations*. Cambridge University Press, 1992. The canonical reference for CPS-based compilation. Covers beta-contraction, constant folding, eta-reduction, inlining, hoisting, closure conversion, and their cascading interaction.

2. **Kennedy, Andrew.** "Compiling with Continuations, Continued." *ICFP 2007*. Refines Appel's approach with a graph-based CPS representation using union-find for efficient eta-reduction.

3. **Cong, Youyou et al.** "Compiling with Continuations, or without? Whatever." *ICFP 2019*. Compares CPS-based and direct-style compilation, discusses administrative redex elimination and local continuation optimizations.

4. **Danvy, Olivier & Filinski, Andrzej.** "Representing Control: A Study of the CPS Transformation." *Mathematical Structures in Computer Science*, 1992. The higher-order CPS transform that avoids generating administrative redexes, using a callback function (meta-continuation) instead of a syntactic continuation.

5. **Appel, Andrew W.** "SSA is Functional Programming." *ACM SIGPLAN Notices*, 1998. + **Kelsey, Richard.** "A Correspondence between Continuation Passing Style and Static Single Assignment Form." *IR '95*, 1995. Establishes the formal equivalence between CPS and SSA, meaning SSA optimization literature applies directly to CPS IRs.

6. **Wingo, Andy.** "CPS Soup" and related posts on *wingolog.org*, 2014-2015. Practical account of how these passes are sequenced in the Guile Scheme compiler, including contification and its interaction with other passes.
