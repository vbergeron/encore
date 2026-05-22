;; This extracted scheme code relies on some additional macros
;; available at http://www.pps.univ-paris-diderot.fr/~letouzey/scheme
(load "macros_extr.scm")


(define eqb (lambda (a) (lambda (b) (if (= a b) `(True) `(False)))))
  
(define leb (lambda (a) (lambda (b) (if (< b a) `(False) `(True)))))
  
(define nat_add (lambda (n) (lambda (m) (+ n m))))

(define nat_sub (lambda (n) (lambda (m) (- n m))))

(define gcd_aux (lambdas (fuel a b)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) a)
     (lambda (fuel~)
     (match (@ eqb a b)
        ((True) a)
        ((False)
          (match (@ leb b a)
             ((True) (@ gcd_aux fuel~ (@ nat_sub a b) b))
             ((False) (@ gcd_aux fuel~ a (@ nat_sub b a)))))))
     fuel)))
  
(define gcd (lambdas (a b) (@ gcd_aux (@ nat_add a b) a b)))

(define main gcd)

