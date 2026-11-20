; encore ir: cps
; Hand-written: comments, quoted symbols, escapes, a match. The printer
; normalizes this layout to the canonical one.
(define |has space|
  (let |x\|y| (bytes "tab\there \"q\" \\ \x00\xff") ; trailing comment
    (let n -42
      (match n 0
        (case () (fin n))
        (case (a b) (let k (cont (r) (fin r)) (encore |has space| (a b) k)))))))
(define main (let k (null-cont) (let p (prim bytes.len |x\|y|) (fin p))))
