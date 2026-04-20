# Fleche

Fleche is the surface language for the Encore system. It provides a direct-style functional syntax that compiles to bytecode for the Encore VM. See the [README](README.md) for the compilation pipeline and CLI usage.

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

Declares constructors with their arities. Five constructors are pre-registered by the VM:

| Name    | Tag | Arity |
|---------|-----|-------|
| `False` | 0   | 0     |
| `True`  | 1   | 0     |
| `Nil`   | 2   | 0     |
| `Cons`  | 3   | 2     |
| `Pair`  | 4   | 2     |

User-declared constructors are assigned tags sequentially starting from `5`. If a `data` declaration re-declares a pre-registered name, the existing registration is kept. An optional leading pipe is allowed for multiline style:

```
data
  | Zero
  | Succ(n)
```

Nullary constructors omit parentheses. Fields in the declaration are named but only their count (arity) matters.

Multiple `data` declarations are allowed; tags continue from the previous declaration's last tag:

```
data Zero | Succ(n)    # tags 5, 6
data Leaf | Node(l, r) # tags 7, 8
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
-7
```

Decimal integers, optionally negative, represented as signed 24-bit values at runtime.

#### String literals

```
"hello"
"line one\nline two"
"tab\there"
```

String literals produce `Bytes` values. The following escape sequences are supported:

| Escape | Byte value |
|--------|------------|
| `\\`   | `\` (0x5C) |
| `\"`   | `"` (0x22) |
| `\n`   | newline (0x0A) |
| `\t`   | tab (0x09) |
| `\0`   | null (0x00) |

Unescaped characters map to one byte each.

#### Variables

```
x
```

#### Lambda

```
x -> body
```

Single-argument function. Currying is expressed by chaining arrows:

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

Plain binding:

```
let x = expr1 in expr2
```

Multiple bindings can be separated by commas. They are desugared into nested lets from right to left:

```
let x = a, y = b in body
# equivalent to: let x = a in let y = b in body
```

#### Destructuring let

A `let` can destructure a constructor value by pattern:

```
let Pair(x, y) = expr in body
```

The constructor must be declared in a `data` block and the number of bindings must match its arity. Destructuring bindings can be chained with commas:

```
let Pair(a, b) = e1, Pair(c, d) = e2 in body
```

#### Recursive let

```
let rec f x = body in rest
```

Binds `f` as a recursive function with parameter `x` in both `body` and `rest`.

#### If (pattern binding)

```
if Cons(h, t) = expr then body else alt
```

Tests whether `expr` matches the given constructor. If it does, the bindings are available in `body`. If it doesn't (the value has a different tag of the same type), `alt` is evaluated instead.

Multiple pattern conditions can be chained with commas — all must succeed for `body` to be reached:

```
if Cons(h1, t1) = xs, Cons(h2, t2) = ys then
  use_both
else
  fallback
```

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
| Zero -> e1
| Succ(pred) -> e2
end
```

Branches are introduced with `|`, the same separator used in `data` declarations. All branches must belong to the same type, and the match must be **exhaustive** — every constructor of the type must be covered. The order of branches does not matter; they are sorted by tag internally. Binders in parentheses are bound to the constructor's fields positionally.

Mixing constructors from different types in a single match is a compile error. Omitting a constructor without a wildcard is also a compile error:

```
data A | B | C

match x
| A -> 1
| B -> 2
end
-- error: non-exhaustive match: missing constructor(s) C
```

A wildcard arm `| _ -> expr` can be used as a default for all unmatched constructors of the type. At least one explicit constructor branch must be present alongside the wildcard. The wildcard must be the last branch:

```
data A | B | C | D

match x
| A -> 1
| D -> 4
| _ -> 0
end
```

The wildcard fills in all missing cases (`B` and `C` above) with the default body.

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

Primitive operations with atom arguments only.

| Builtin | Args | Returns |
|---------|------|---------|
| `add`, `sub`, `mul` | 2 integers | integer |
| `eq`, `lt` | 2 integers | `True` (tag 1) or `False` (tag 0) |
| `int_byte` | 1 integer (0–255) | single-byte `Bytes` value |
| `bytes_len` | 1 `Bytes` | integer (length) |
| `bytes_get` | `Bytes`, integer index | integer (byte value) |
| `bytes_concat` | 2 `Bytes` | `Bytes` |
| `bytes_slice` | `Bytes`, start, length | `Bytes` |
| `bytes_eq` | 2 `Bytes` | `True` or `False` |

Example using comparisons with match:

```
data False | True

define main as
  let r = builtin lt 3 5 in
  match r
  | False -> 0
  | True -> 1
  end
```
