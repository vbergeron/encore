; Tail-recursive sum with accumulator via Pair packing
; sum(1000, 0) = 500500

(define sum (lambda (p)
  (match p
    ((Pair n acc)
      (if (= n 0)
        acc
        (sum `(Pair ,(- n 1) ,(+ acc n))))))))

(define main (sum `(Pair ,1000 ,0)))
