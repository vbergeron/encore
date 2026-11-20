# IR text formats

Every intermediate representation has a textual S-expression form
(`crates/encore_compiler/src/ir/text/`). Printing then parsing gives back the
same module, so you can stop the pipeline at any stage, inspect or edit the
text, and resume compilation from it.

```bash
# Stop after a stage and print its IR to stdout
encore compile fleche prog.fleche --emit cps-optimize > prog.cps

# Resume from a file produced by that stage (runs the following passes)
encore compile ir prog.cps --from cps-optimize --out out

# Run only part of the pipeline: dsi text in, unoptimized cps text out
encore compile ir prog.dsi --from dsi-resolve --emit cps-transform
```

`--emit` and `--from` take a **stage**, named after the pass that produced
the IR. `--from X` means "this file is the output of X". Compilation resumes
with the pass after X, so `--emit X` followed by `--from X` produces the same
bytecode as compiling straight through.

| Stage           | IR    | Produced by                          |
|-----------------|-------|--------------------------------------|
| `parse`         | `ds`  | the frontend (Fleche / Scheme)       |
| `ds-uncurry`    | `ds`  | `ds_uncurry`                         |
| `dsi-resolve`   | `dsi` | `dsi_resolve`                        |
| `cps-transform` | `cps` | `cps_transform`                      |
| `cps-optimize`  | `cps` | `cps_optimize` (identity with `--cps-optimize off`) |
| `asm-resolve`   | `asm` | `asm_resolve`                        |
| `asm-peephole`  | `asm` | `asm_peephole`                       |

From Rust code, use `Module::to_text()` / `Module::from_text()` on each IR, or
`pipeline::{Ir, Stage, run_stages, compile_from}`.

When you resume from IR text, constructor names are not available, because
only the frontend knows them. Global names come from the `define` names in
`ds`, `dsi` and `cps`. The `asm` IR identifies defines by index only.

## Lexical syntax (shared)

- `; comment` runs to the end of the line.
- Integers: `42`, `-7`.
- Strings are byte literals: `"text"`, with escapes `\" \\ \n \t \r \xNN`.
- Symbols: any run of characters other than whitespace and `( ) " ; |`
  that does not start like a number. Other names are written `|like this|`,
  with escapes `\| \\`.
- A file is a sequence of `(define NAME BODY)` forms, in global order.

Primitives (`PRIM`): `int.add int.sub int.mul int.eq int.lt int.byte int.div
int.mod int.sub_sat int.le int.and int.or int.xor int.shl int.shr bytes.len
bytes.get bytes.concat bytes.slice bytes.eq`.

`TAG`, field indices, extern slots, globals, captures and registers must fit
their target widths (`u8` or `u16`). Integer literals must fit `i32`.

## `ds`: direct style, named

```
E ::= x                               variable
    | INT
    | (lambda (x ...) E)
    | (app E E ...)                   application
    | (let x E E)
    | (letrec f x E E)                f(x) = E₁ in E₂
    | (ctor TAG E ...)
    | (field E IDX)
    | (match E BASE (case (x ...) E) ...)   case i matches tag BASE+i
    | (bytes "...")
    | (prim PRIM E ...)
    | (extern SLOT)
```

## `dsi`: direct style, de Bruijn indexed

The same forms as `ds`, without binder names:

```
E ::= (var N) | INT | (lambda ARITY E) | (app E E ...) | (let E E)
    | (letrec E E) | (ctor TAG E ...) | (field E IDX)
    | (match E BASE (case ARITY E) ...) | (bytes "...") | (prim PRIM E ...)
    | (extern SLOT)
```

Index 0 is the innermost binder. Globals sit below every local binder, in
define order. `letrec` binds `f` and then its parameter inside its body, and
only `f` inside the rest.

## `cps`: continuation-passing style

```
E ::= (let x V E)
    | (letrec f (x ...) k E E)        f(x ...; k) = E₁ in E₂
    | (encore f (x ...) k)            tail call
    | (match x BASE (case (y ...) E) ...)
    | (fin x)

V ::= x | INT | (cont (x ...) E) | (null-cont)
    | (ctor TAG x ...) | (field x IDX) | (bytes "...")
    | (prim PRIM x ...) | (extern SLOT)
```

## `asm`: registers

Registers are written `self`, `cont`, `a1`…`a8`, `null`, or `r0`…`r255`.
Defines are numbered: `(define GLOBAL E)`.

```
E ::= (let R V E)
    | (letrec R (R ...) E E)          closure capturing R ..., body, rest
    | (encore R (R ...) R)
    | (match R BASE (case ARITY UNPACK_BASE E) ...)
    | (fin R)

V ::= R | INT | (capture N) | (global N) | (cont-lam (R ...) E)
    | (ctor TAG R ...) | (field R IDX) | (bytes "...")
    | (prim PRIM R ...) | (extern SLOT)
```

## Example

```
; encore ir: cps

(define main
  (let _i0 1000
  (let _i1 0
  (let _c2 (ctor 4 _i0 _i1)
  (let _k3 (cont (_r4) (fin _r4)) (encore sum (_c2) _k3))))))
```

The printer lays out a form on one line when it fits in 80 columns. Otherwise
it breaks the form's arguments onto lines indented by two. The exception is
the last argument of `let` and `letrec` (the body, or the rest after a
`letrec`): it goes at the binding's own column, so a chain of bindings reads
top to bottom. Indentation then grows only for nested closures, function
bodies and match cases. Layout is not significant to the readers.
