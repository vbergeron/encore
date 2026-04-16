; Higher-order list combinators: map, filter, sum
; sum(filter(>5, map(double, [1..50])))

(define make (lambda (n)
  (if (= n 0)
    `(Nil)
    `(Cons ,n ,(make (- n 1))))))

(define map (lambda (f)
  (letrec ((go (lambda (lst)
    (match lst
      ((Nil) `(Nil))
      ((Cons h t)
        (let ((fh (f h)))
          `(Cons ,fh ,(go t))))))))
    go)))

(define filter (lambda (p)
  (letrec ((go (lambda (lst)
    (match lst
      ((Nil) `(Nil))
      ((Cons h t)
        (let ((keep (p h)))
        (let ((rest (go t)))
          (if keep
            `(Cons ,h ,rest)
            rest))))))))
    go)))

(define sum (lambda (lst)
  (match lst
    ((Nil) 0)
    ((Cons h t) (+ h (sum t))))))

(define double (lambda (x) (* x 2)))

(define is_big (lambda (x) (< 5 x)))

(define main
  (sum ((filter is_big) ((map double) (make 50)))))
