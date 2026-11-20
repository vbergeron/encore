Every stage round-trips the Rust compiler's text byte for byte:

  $ for st in parse ds-uncurry dsi-resolve cps-transform cps-optimize asm-resolve asm-peephole; do
  >   encore-passes fib.$st --from $st | cmp - fib.$st && echo "$st ok"
  > done
  parse ok
  ds-uncurry ok
  dsi-resolve ok
  cps-transform ok
  cps-optimize ok
  asm-resolve ok
  asm-peephole ok

The implemented passes (cps-optimize and asm-peephole, both the identity
for now) run between stages of the same IR:

  $ encore-passes fib.cps-transform --from cps-transform --emit cps-optimize | cmp - fib.cps-transform
  $ encore-passes --from asm-resolve --emit asm-peephole < fib.asm-resolve | cmp - fib.asm-resolve

The printer normalizes hand-written text: comments go, layout is canonical,
and quoted symbols and byte strings keep their escapes:

  $ encore-passes lexical.cps --from cps-optimize
  ; encore ir: cps
  
  (define |has space|
    (let |x\|y|
      (bytes "tab\there \"q\" \\ \x00\xff")
    (let n -42
    (match n 0 (case () (fin n))
      (case (a b) (let k (cont (r) (fin r)) (encore |has space| (a b) k)))))))
  
  (define main (let k (null-cont) (let p (prim bytes.len |x\|y|) (fin p))))

Passes not written yet, and other errors:

  $ encore-passes fib.parse --from parse --emit asm-peephole
  encore-passes: fib.parse: ds-uncurry is not implemented yet
  [1]
  $ encore-passes fib.cps-optimize --from cps-optimize --emit parse
  encore-passes: fib.cps-optimize: cannot go back from cps-optimize to parse
  [1]
  $ encore-passes fib.cps-optimize --from asm-resolve
  encore-passes: fib.cps-optimize: expected an integer, found symbol `fib`
  [1]
  $ echo '(define x (let y 2147483648 (fin y)))' | encore-passes --from cps-transform
  encore-passes: <stdin>: integer literal out of range
  [1]
  $ echo '(define x (fin y z))' | encore-passes --from cps-transform
  encore-passes: <stdin>: `fin` expects 1 argument(s), found 2
  [1]
  $ printf '(define x\n  (fin "y))' | encore-passes --from cps-transform
  encore-passes: <stdin>: line 2: unterminated `"`
  [1]
  $ encore-passes fib.parse --from parsed
  encore-passes: --from: unknown stage `parsed` (expected one of: parse, ds-uncurry, dsi-resolve, cps-transform, cps-optimize, asm-resolve, asm-peephole)
  [1]
