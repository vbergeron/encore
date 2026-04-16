; Naive recursive fibonacci — deep recursion, heavy arithmetic
; fib(15) = 610

(define fib (lambda (n)
  (if (< n 2)
    n
    (+ (fib (- n 1)) (fib (- n 2))))))

(define main (fib 15))
