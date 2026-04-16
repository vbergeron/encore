;; This extracted scheme code relies on some additional macros
;; available at http://www.pps.univ-paris-diderot.fr/~letouzey/scheme
(load "macros_extr.scm")


(define add (lambdas (n m) (+ n m)))
  
(define eqb (lambdas (n m) (if (= n m) `(True) `(False))))
  
(define leb (lambdas (n m) (if (< m n) `(False) `(True))))
  
(define shift (lambdas (d c t)
  (match t
     ((Var n)
       (match (@ leb c n)
          ((True) `(Var ,(@ add n d)))
          ((False) `(Var ,n))))
     ((Abs body) `(Abs ,(@ shift d `((lambda (n) (+ n 1)) ,c) body)))
     ((App t1 t2) `(App ,(@ shift d c t1) ,(@ shift d c t2))))))
  
(define subst (lambdas (j s t)
  (match t
     ((Var n) (match (@ eqb j n)
                 ((True) s)
                 ((False) `(Var ,n))))
     ((Abs body) `(Abs
       ,(@ subst `((lambda (n) (+ n 1)) ,j)
          (@ shift `((lambda (n) (+ n 1)) ,`(0)) `(0) s) body)))
     ((App t1 t2) `(App ,(@ subst j s t1) ,(@ subst j s t2))))))
  
(define beta (lambdas (body arg)
  (@ shift `((lambda (n) (+ n 1)) ,`(0)) `(0)
    (@ subst `(0) (@ shift `((lambda (n) (+ n 1)) ,`(0)) `(0) arg) body))))

(define whnf (lambdas (fuel t)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) `(None))
     (lambda (fuel~)
     (match t
        ((Var _) `(Some ,t))
        ((Abs _) `(Some ,t))
        ((App t1 t2)
          (match (@ whnf fuel~ t1)
             ((Some t1~)
               (match t1~
                  ((Var _) `(Some ,`(App ,t1~ ,t2)))
                  ((Abs body) (@ whnf fuel~ (@ beta body t2)))
                  ((App _ _) `(Some ,`(App ,t1~ ,t2)))))
             ((None) `(None))))))
     fuel)))
  
(define nf (lambdas (fuel t)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) `(None))
     (lambda (fuel~)
     (match (@ whnf fuel~ t)
        ((Some t~)
          (match t~
             ((Var _) `(Some ,t~))
             ((Abs body)
               (match (@ nf fuel~ body)
                  ((Some body~) `(Some ,`(Abs ,body~)))
                  ((None) `(None))))
             ((App t1 t2)
               (match (@ nf fuel~ t1)
                  ((Some t1~)
                    (match (@ nf fuel~ t2)
                       ((Some t2~) `(Some ,`(App ,t1~ ,t2~)))
                       ((None) `(None))))
                  ((None) `(None))))))
        ((None) `(None))))
     fuel)))
  
(define church (lambda (n)
  (let ((go
    (letrec ((go
            (lambda (n0)
            ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
               (lambda (_) `(Var ,`(0)))
               (lambda (m) `(App ,`(Var ,`((lambda (n) (+ n 1)) ,`(0)))
               ,(go m)))
               n0))))
            go)))
    `(Abs ,`(Abs ,(go n))))))

(define church_add `(Abs ,`(Abs ,`(Abs ,`(Abs ,`(App ,`(App ,`(Var
  ,`((lambda (n) (+ n 1)) ,`((lambda (n) (+ n 1)) ,`((lambda (n) (+ n 1))
  ,`(0))))) ,`(Var ,`((lambda (n) (+ n 1)) ,`(0)))) ,`(App ,`(App ,`(Var
  ,`((lambda (n) (+ n 1)) ,`((lambda (n) (+ n 1)) ,`(0)))) ,`(Var
  ,`((lambda (n) (+ n 1)) ,`(0)))) ,`(Var ,`(0)))))))))

(define church_mul `(Abs ,`(Abs ,`(Abs ,`(App ,`(Var ,`((lambda (n) (+ n 1))
  ,`((lambda (n) (+ n 1)) ,`(0)))) ,`(App ,`(Var ,`((lambda (n) (+ n 1))
  ,`(0))) ,`(Var ,`(0))))))))

(define read_church (lambda (t)
  (match t
     ((Var _) `(None))
     ((Abs t0)
       (match t0
          ((Var _) `(None))
          ((Abs body)
            (letrec ((count
                    (lambda (t1)
                    (match t1
                       ((Var n)
                         ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
                            (lambda (_) `(Some ,`(0)))
                            (lambda (_) `(None))
                            n))
                       ((Abs _) `(None))
                       ((App t2 rest)
                         (match t2
                            ((Var n)
                              ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
                                 (lambda (_) `(None))
                                 (lambda (n0)
                                 ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
                                    (lambda (_)
                                    (match (count rest)
                                       ((Some n1) `(Some
                                         ,`((lambda (n) (+ n 1)) ,n1)))
                                       ((None) `(None))))
                                    (lambda (_) `(None))
                                    n0))
                                 n))
                            ((Abs _) `(None))
                            ((App _ _) `(None))))))))
                    (count body)))
          ((App _ _) `(None))))
     ((App _ _) `(None)))))
  
(define test_add (lambdas (a b fuel)
  (match (@ nf fuel `(App ,`(App ,church_add ,(church a)) ,(church b)))
     ((Some t) (read_church t))
     ((None) `(None)))))

(define test_mul (lambdas (a b fuel)
  (match (@ nf fuel `(App ,`(App ,church_mul ,(church a)) ,(church b)))
     ((Some t) (read_church t))
     ((None) `(None)))))

