; Church numerals — closure-heavy, tests capture and GC
; to_int(eight) = 8

(define zero (lambdas (f x) x))

(define succ (lambda (n) (lambdas (f x) (f (@ n f x)))))

(define add (lambdas (a b) (lambdas (f x) (@ a f (@ b f x)))))

(define to_int (lambda (n) (@ n (lambda (x) (+ x 1)) 0)))

(define main
  (let ((two (succ (succ zero))))
  (let ((four (@ add two two)))
  (let ((eight (@ add four four)))
    (to_int eight)))))
