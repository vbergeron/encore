; Ackermann function on Peano naturals — explosive recursion
; ack(3, 3) = 61

(define ack (lambda (p)
  (match p
    ((Pair m n)
      (match m
        ((Zero) `(Succ ,n))
        ((Succ pm)
          (match n
            ((Zero) (ack `(Pair ,pm ,`(Succ ,`(Zero)))))
            ((Succ pn)
              (let ((inner (ack `(Pair ,m ,pn))))
                (ack `(Pair ,pm ,inner)))))))))))

(define to_int (lambda (n)
  (match n
    ((Zero) 0)
    ((Succ p) (+ (to_int p) 1)))))

(define main
  (to_int (ack `(Pair ,`(Succ ,`(Succ ,`(Succ ,`(Zero))))
                      ,`(Succ ,`(Succ ,`(Succ ,`(Zero))))))))
