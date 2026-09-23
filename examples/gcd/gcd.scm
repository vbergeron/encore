;; This extracted scheme code relies on some additional macros
;; available at http://www.pps.univ-paris-diderot.fr/~letouzey/scheme
(load "macros_extr.scm")


(define add (lambda (a) (lambda (b) (+ a b))))
  
(define sub (lambda (a) (lambda (b) (int-sub-sat a b))))
  
(define eqb (lambda (a) (lambda (b) (= a b))))
  
(define leb (lambda (a) (lambda (b) (<= a b))))
  
(define gcd_aux (lambdas (fuel a b)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) a)
     (lambda (fuel~)
     (match (@ eqb a b)
        ((True) a)
        ((False)
          (match (@ leb b a)
             ((True) (@ gcd_aux fuel~ (@ sub a b) b))
             ((False) (@ gcd_aux fuel~ a (@ sub b a)))))))
     fuel)))
  
(define gcd (lambdas (a b) (@ gcd_aux (@ add a b) a b)))

(define main gcd)

(define check
  (@ gcd `((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1))
    ,`(0)))))))))))))))))))))))))))))))))))))))))))))))))
    `((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1)) ,`((lambda (x) (+ x 1))
    ,`(0)))))))))))))))))))))

