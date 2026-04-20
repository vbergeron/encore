# Scheme Frontend

The Scheme frontend (`encore_scheme`) consumes Rocq-extracted `.scm` files. It is not a general-purpose Scheme implementation — it recognizes a fixed set of special forms with no macro expander, and uses non-standard conventions for constructors and multi-argument functions that match the output of Rocq's Scheme extraction.

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
| `(int->byte x)` | integer 0–255 to single-byte string |
| `(bytes-len s)` | byte string length |
| `(bytes-get s i)` | byte at index |
| `(bytes-concat a b)` | concatenate byte strings |
| `(bytes-slice s i n)` | substring from index, length n |
| `(bytes-eq a b)` | byte string equality (returns `True`/`False` constructor) |

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
