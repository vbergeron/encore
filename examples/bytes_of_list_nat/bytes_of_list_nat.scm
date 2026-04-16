; Minimal reproducer for the rewrite-inlining miscompilation.
;
; When --cps-optimize-rewrite-inlining is on, the inliner drops the
; `lst` capture from the CPS continuation that receives the init
; accumulator, so fold_left sees garbage instead of the list.
;
; Trigger conditions:
;   1. A 3+ arg wrapper calls fold_left
;   2. The init accumulator is computed by a match + function call
;      on one arg (creating a CPS continuation that must capture the
;      remaining args)
;   3. The step function uses acc (so fold_left's body is non-trivial)
;
; Expected: 43   (id 42 = 42, then 42 + 1 = 43)

(define fold_left (lambdas (f l a0)
  (match l
     ((Nil) a0)
     ((Cons b l0) (@ fold_left f l0 (@ f a0 b))))))

(define go (lambdas (ops n0 lst)
  (@ fold_left
    (lambdas (acc n) (+ acc n))
    lst
    (match ops
      ((Box h) (h n0))))))

(define id (lambda (x) x))

(define main (@ go `(Box ,id) 42 `(Cons ,1 ,`(Nil))))
