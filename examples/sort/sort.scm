;; This extracted scheme code relies on some additional macros
;; available at http://www.pps.univ-paris-diderot.fr/~letouzey/scheme
(load "macros_extr.scm")


(define length (lambda (l)
  (match l
     ((Nil) `(0))
     ((Cons _ l~) `((lambda (n) (+ n 1)) ,(length l~))))))
  
(define app (lambdas (l m)
  (match l
     ((Nil) m)
     ((Cons a l1) `(Cons ,a ,(@ app l1 m))))))
  
(define leb (lambdas (n m) (if (< m n) `(False) `(True))))
  
(define insert (lambdas (n l)
  (match l
     ((Nil) `(Cons ,n ,`(Nil)))
     ((Cons h t)
       (match (@ leb n h)
          ((True) `(Cons ,n ,l))
          ((False) `(Cons ,h ,(@ insert n t))))))))
  
(define insertion_sort (lambda (l)
  (match l
     ((Nil) `(Nil))
     ((Cons h t) (@ insert h (insertion_sort t))))))
  
(define merge (lambdas (fuel l1 l2)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) (@ app l1 l2))
     (lambda (fuel~)
     (match l1
        ((Nil) l2)
        ((Cons h1 t1)
          (match l2
             ((Nil) l1)
             ((Cons h2 t2)
               (match (@ leb h1 h2)
                  ((True) `(Cons ,h1 ,(@ merge fuel~ t1 l2)))
                  ((False) `(Cons ,h2 ,(@ merge fuel~ l1 t2)))))))))
     fuel)))
  
(define split (lambda (l)
  (match l
     ((Nil) `(Pair ,`(Nil) ,`(Nil)))
     ((Cons x l0)
       (match l0
          ((Nil) `(Pair ,`(Cons ,x ,`(Nil)) ,`(Nil)))
          ((Cons y rest)
            (match (split rest)
               ((Pair l1 l2) `(Pair ,`(Cons ,x ,l1) ,`(Cons ,y ,l2))))))))))
  
(define merge_sort_aux (lambdas (fuel l)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) l)
     (lambda (fuel~)
     (match l
        ((Nil) `(Nil))
        ((Cons x l0)
          (match l0
             ((Nil) `(Cons ,x ,`(Nil)))
             ((Cons _ _)
               (match (split l)
                  ((Pair l1 l2)
                    (@ merge fuel (@ merge_sort_aux fuel~ l1)
                      (@ merge_sort_aux fuel~ l2)))))))))
     fuel)))
  
(define merge_sort (lambda (l)
  (let ((n (length l))) (@ merge_sort_aux n l))))

(define seq (lambdas (start len)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) `(Nil))
     (lambda (n) `(Cons ,start ,(@ seq `((lambda (n) (+ n 1)) ,start) n)))
     len)))
  
(define rev_range (lambda (n)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) `(Nil))
     (lambda (m) `(Cons ,n ,(rev_range m)))
     n)))
  
(define sort_seq (lambda (n) (merge_sort (rev_range n))))

(define sort_insert_seq (lambda (n) (insertion_sort (rev_range n))))

