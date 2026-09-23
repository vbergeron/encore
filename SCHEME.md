# Scheme Frontend

The Scheme frontend (`encore_scheme`) consumes Rocq-extracted `.scm` files. To produce them, extract through `rocq/ExtrEncore.v`; see [Extracting from Rocq](#extracting-from-rocq). It is not a general-purpose Scheme implementation — it recognizes a fixed set of special forms with no macro expander, and uses non-standard conventions for constructors and multi-argument functions that match the output of Rocq's Scheme extraction.

## S-expression surface

The parser reads a minimal S-expression syntax:

- **Atoms**: any run of non-delimiter, non-whitespace characters.
- **Lists**: `(` ... `)`.
- **Strings**: double-quoted, raw bytes (no escape sequences).
- **Comments**: `;` to end of line.
- **Integers**: decimal (`42`, `-3`) and hexadecimal (`0x1A`, `0X1a`).

Reader sugar:

| Sugar | Expansion |
|-------|-----------|
| `'x` | `(quote x)` |
| `` `x `` | `(quasiquote x)` |
| `,x` | `(unquote x)` |

There is no `#t`/`#f`, no `#(...)` vectors, and no `,@` (unquote-splicing).

## Top-level forms

A program is a sequence of top-level list forms. Only the following heads are recognized:

### `load` (ignored)

```scheme
(load "macros.scm")
```

Silently skipped. There is no file inclusion.

### `define`

```scheme
(define name body)
```

Binds `name` to the result of `body`. There is no shorthand `(define (f x) ...)` — the name must be an atom.

### `define` with `extern` body

Foreign function binding with explicit slot assignment:

```scheme
(define sign (extern (slot 3) bytes))
```

`(extern (slot N))` binds directly to slot `N`. With trailing parameter names, a curried lambda wrapper is generated that packs the arguments into a constructor and calls the extern.

## Expressions

### `lambda`

```scheme
(lambda (x) body)
```

Exactly **one** parameter. Multi-parameter lambdas must use `lambdas`.

### `lambdas`

```scheme
(lambdas (a b c) body)
```

Multi-parameter function. Lowered to nested single-parameter lambdas:

```scheme
(lambdas (a b c) body)
;; becomes: (lambda (a) (lambda (b) (lambda (c) body)))
```

### `@` (application)

```scheme
(@ f a b)
```

Explicit application form. With zero extra arguments, returns the function value unchanged. With one argument, a unary application. With two or more, an n-ary application (the compiler's uncurry pass may optimize these into saturated calls).

Bare S-expression application `(f x)` also works — any list whose head is not a recognized special form is treated as application.

### `if`

```scheme
(if cond then-expr else-expr)
```

Lowered to a two-case match on the condition: tag 0 (`False`) selects the else branch, tag 1 (`True`) selects the then branch.

### `let`

```scheme
(let ((x e1) (y e2)) body)
```

Multiple bindings are desugared into nested lets from right to left.

### `letrec`

```scheme
(letrec ((f (lambda (x) body))) rest)
```

Only a **single binding** is supported. If the bound value is a `lambda`, it maps directly to a recursive function. If it is anything else, the frontend eta-expands it: `(letrec ((f g)) ...)` becomes the equivalent of `let rec f __eta = g __eta in ...`.

### `match`

```scheme
(match scrutinee
  ((Nil) 0)
  ((Cons h t) (+ h (fold t))))
```

Each case is `((Tag binder ...) body)`. The constructor name is resolved to a numeric tag via the shared constructor registry. Cases are sorted by tag, and any gaps in the tag range are filled with an unreachable error stub (an infinite loop).

### `quote`

```scheme
'Nil          ; -> Nil constructor (nullary)
'()           ; -> Nil
'(SomeTag)    ; -> SomeTag constructor (nullary, ignores list tail)
```

Very limited. `quote` on an atom produces a nullary constructor. `quote` on an empty list produces `Nil`. `quote` on a list takes only the head atom as a nullary constructor name — subforms are ignored.

### Quasiquote and unquote

Quasiquote is the primary way to **build constructor values** with fields:

```scheme
`(Cons ,x ,y)       ; -> Cons(x, y)
`(Pair ,a ,(f b))   ; -> Pair(a, f(b))
`(True)             ; -> True (nullary)
```

When the head of a quasiquoted list is a non-numeric atom, it is treated as a constructor name. `,expr` (unquote) subforms become the constructor's field values. Nested quasiquotes are handled recursively.

If the head is numeric or not an atom, the quasiquoted list is treated as an application instead.

### `error`

```scheme
(error)
```

Produces an infinite loop (`let __err = (lambda (x) x) in (__err __err)`). Used as a crash/unreachable marker.

### Primitives

| Form | Operation |
|------|-----------|
| `(+ a b)` | integer addition |
| `(- a b)` | integer subtraction |
| `(* a b)` | integer multiplication |
| `(= a b)` | integer equality (returns `True`/`False` constructor) |
| `(< a b)` | integer less-than (returns `True`/`False` constructor) |
| `(<= a b)` | integer less-or-equal (returns `True`/`False` constructor) |
| `(int-div a b)` | truncating division; `(int-div a 0)` = `0` |
| `(int-mod a b)` | truncating remainder; `(int-mod a 0)` = `a` |
| `(int-sub-sat a b)` | `nat` subtraction: `a - b`, or `0` if `a <= b` |
| `(int-and a b)` | bitwise and |
| `(int-or a b)` | bitwise or |
| `(int-xor a b)` | bitwise xor |
| `(int-shl a b)` | left shift; traps with `IntOverflow` if the result leaves the 24-bit range |
| `(int-shr a b)` | logical right shift (`b` outside `0..24` gives `0`) |
| `(int->byte x)` | integer 0–255 to single-byte string |
| `(bytes-len s)` | byte string length |
| `(bytes-get s i)` | byte at index |
| `(bytes-concat a b)` | concatenate byte strings |
| `(bytes-slice s i n)` | substring from index, length n |
| `(bytes-eq a b)` | byte string equality (returns `True`/`False` constructor) |

Integer semantics are those of the VM opcodes; see [VM.md](VM.md#integer-operations).

With `nat` extracted to `integer`, `rocq/ExtrEncore.v` maps the `nat` library functions (`Nat.div`, `Nat.modulo`, `Nat.land`, `Nat.shiftl`, ...) onto these primitives instead of running them as extracted Gallina. See [Extracting from Rocq](#extracting-from-rocq). Do not map `Z.div`/`Z.modulo` to `int-div`/`int-mod`: they floor, and differ on negative operands.

### Otherwise: application

Any list form whose head is not a recognized keyword is treated as function application. `(f x)` is unary application, `(f x y z)` is n-ary.

## Constructors

There is no `data` declaration in the Scheme frontend. Constructors are registered on first use — the first time a name appears as a tag in a `match` case, a `quote`, or a quasiquote head, it is assigned a numeric tag. The same five names are pre-registered as in Fleche (`False`, `True`, `Nil`, `Cons`, `Pair`); user constructors get tags from 5 upward.

Constructor arity is inferred from usage: a `match` case `((Cons h t) ...)` registers `Cons` with arity 2; a quasiquote `` `(Cons ,x ,y) `` does the same. If the same constructor appears with different arities in different locations, the first registration wins.

## Differences from standard Scheme

This is **not** an R5RS/R7RS implementation. Key restrictions:

- **No macros** — `define-syntax`, `syntax-rules`, `let-syntax` are not recognized.
- **No `begin`**, `set!`, `cond`, `case`, `do`, `when`, `unless`, or any other standard special form not listed above.
- **`lambda` takes exactly one parameter** — use `lambdas` for multiple.
- **`letrec` supports a single binding only**.
- **No boolean literals** — use the `True`/`False` constructors via quasiquote.
- **No `define` shorthand** — `(define (f x) ...)` is not supported; write `(define f (lambda (x) ...))`.
- **`load` is ignored** — there is no module or file inclusion system.
- **`quote` is very restricted** — it cannot build constructors with fields; use quasiquote for that.

## Extracting from Rocq

The supported way to extract a Rocq program for Encore is the `Encore.Extraction` theory in [`rocq/`](rocq/) (opam package `rocq-encore`, Rocq 9.1, dune ≥ 3.21). It contains:

| Module | Contents |
|--------|----------|
| `ExtrEncore` | Sets `Extraction Language Scheme`. Maps `nat` to VM integers and its operations to primitives, and pins the `bool`/`list`/`prod` constructor names. |
| `ExtrEncoreBytes` | An abstract `bytes` type whose operations are VM byte-string primitives, plus `bytes_of_string` for `string` literals. |
| `ExtrEncoreInput` | The extern idiom: `input_byte : nat -> nat` realised by `(extern (slot 0) i)`, and `read_bytes`. |

```coq
From Encore.Extraction Require Import ExtrEncore.
Require Import MyProgram.
Extraction "my_program.scm" MyProgram.main.
```

```bash
encore compile scheme my_program.scm --out out
```

To depend on it from another dune project, pin the package: `opam pin add rocq-encore git+https://github.com/vbergeron/encore`, then add `Encore.Extraction` to your theory's `(theories ...)`. Inside this repository, `dune build` builds the theory and re-extracts the examples (`examples/gcd`, `examples/digits`). The extracted `.scm` files are promoted into the source tree and committed, so building the firmware does not need Rocq. CI rebuilds them, fails if they differ from the committed ones, and compiles and runs them with `encore`.

### What `ExtrEncore` maps

- **`nat`** becomes `integer`: `O` is `0`, `S` is `(+ x 1)`, and a `match` on `nat` is an eliminator that tests for zero.
- **`nat` operations**: `add`, `mul`, `sub` (`int-sub-sat`), `pred`, `min`, `max`, `eqb`, `leb`, `ltb`, `even`, `odd`, `div`, `modulo`, `div2`, `land`, `lor`, `lxor`, `shiftl` and `shiftr` all become primitives. Each one is mapped **twice**, once as `Init.Nat.*` and once as `PeanoNat.Nat.*`. The notations `+`, `*`, `-` unfold to `Init.Nat`, but once `Arith` is imported, `Nat.eqb`, `=?`, `<=?`, `/`, `mod`, ... resolve to `PeanoNat.Nat`. Its constants are aliases that extraction treats as distinct. If only one set is mapped, the directives silently do not apply to the other. `=?` then extracts to the recursive Gallina `eqb`. It is still correct through the `nat` eliminator, but it takes time linear in its arguments instead of one instruction, and nothing warns about it.
- **`bool`, `list`, `prod`** keep their constructor representation, with names pinned to the pre-registered constructors: `False`/`True` (tags 0/1), `Nil`/`Cons` (2/3), `Pair` (4). Pinning matters. Without it, Rocq renames a constructor whose name clashes with one from another inductive. For example, `Decimal.uint` also has a `Nil`, which would turn `list`'s `nil` into `Nil1`. `crates/encore_scheme/tests/rocq_extraction.rs` checks these names and tags against `CtorRegistry`.
- **`ascii`, `string`** keep their default extraction, as recommended in #7. The frontend folds every closed `String`/`Ascii` chain into a byte-string literal (see `fold_string_literals`), and `ExtrEncoreBytes.bytes_of_string` is the identity on it.

`nat` literals above 5000 are not extracted as successor chains. Rocq abstracts them as `Nat.of_num_uint` applied to a decimal digit list, which is correct but converted at run time and large. Keep literals below 5000 or build them arithmetically.

## Trust assumptions

Every directive in `rocq/` replaces a Gallina definition with unproven Scheme. A program extracted through it is correct only if these preconditions hold:

| Directive | Precondition | If violated |
|-----------|--------------|-------------|
| `nat` → `integer`, and all `nat` operations | Every `nat` the program builds stays below 2^23 (8,388,608). `nat` is unbounded; VM integers are 24-bit. | The VM traps with `IntOverflow` on the operation that leaves the range, so it never returns a wrong value. It fails instead of returning the Gallina result. |
| `bool`/`list`/`prod` names | No other constructor in the extracted program is named `False`, `True`, `Nil`, `Cons` or `Pair`. The frontend resolves tags by name. | Constructors of different types share a tag. This is harmless while their arities agree (for example `Decimal.uint`'s `Nil`). |
| `ExtrEncoreBytes.bytes_of_string` (identity) | Applied only to `string` literals, or to constants defined as literals. | A `string` computed at run time is still a `String` constructor chain, not a byte string. |
| `string`/`ascii` literal folding | The program never computes on `string` or `ascii` values: no `match`, no `String.append`/`length`/`eqb`, ... | A folded literal is a byte string, and matching on it as a `String` constructor is miscompiled. |
| `ExtrEncoreBytes.bytes_get b i` | `i < bytes_len b` | The VM does not check the bound and reads past the end. |
| `ExtrEncoreBytes.byte_of_nat n` | `n <= 255` | The VM traps with `ByteRange`. |
| `ExtrEncoreBytes` axioms | The VM primitives satisfy the stated equations (`bytes_len_concat`, `bytes_eqb_spec`, ...). | Proofs that use the axioms say nothing about the VM. |
| `ExtrEncoreInput.input_byte` (`extern (slot 0)`) | The host registers a function in slot 0 that returns a value in `[0, 255]` for every index read, and the same value every time for the same index. | Anything: the extern is outside Rocq's model. |
