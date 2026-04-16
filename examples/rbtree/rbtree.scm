;; This extracted scheme code relies on some additional macros
;; available at http://www.pps.univ-paris-diderot.fr/~letouzey/scheme
(load "macros_extr.scm")


(define add (lambdas (n m) (+ n m)))
  
(define ltb (lambdas (n m) (if (< n m) `(True) `(False))))

(define max (lambdas (n m)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) m)
     (lambda (n~)
     ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
        (lambda (_) n)
        (lambda (m~) `((lambda (n) (+ n 1)) ,(@ max n~ m~)))
        m))
     n)))
  
(define member (lambdas (x t)
  (match t
     ((Leaf) `(False))
     ((Node _ l k r)
       (match (@ ltb x k)
          ((True) (@ member x l))
          ((False)
            (match (@ ltb k x)
               ((True) (@ member x r))
               ((False) `(True)))))))))
  
(define balance (lambdas (c l k r)
  (match c
     ((Red) `(Node ,c ,l ,k ,r))
     ((Black)
       (match l
          ((Leaf)
            (match r
               ((Leaf) `(Node ,c ,l ,k ,r))
               ((Node c0 b y d)
                 (match c0
                    ((Red)
                      (match b
                         ((Leaf)
                           (match d
                              ((Leaf) `(Node ,c ,l ,k ,r))
                              ((Node c1 c_ z d0)
                                (match c1
                                   ((Red) `(Node ,`(Red) ,`(Node ,`(Black) ,l
                                     ,k ,b) ,y ,`(Node ,`(Black) ,c_ ,z
                                     ,d0)))
                                   ((Black) `(Node ,c ,l ,k ,r))))))
                         ((Node c1 b0 y0 c_)
                           (match c1
                              ((Red) `(Node ,`(Red) ,`(Node ,`(Black) ,l ,k
                                ,b0) ,y0 ,`(Node ,`(Black) ,c_ ,y ,d)))
                              ((Black)
                                (match d
                                   ((Leaf) `(Node ,c ,l ,k ,r))
                                   ((Node c2 c_0 z d0)
                                     (match c2
                                        ((Red) `(Node ,`(Red) ,`(Node
                                          ,`(Black) ,l ,k ,b) ,y ,`(Node
                                          ,`(Black) ,c_0 ,z ,d0)))
                                        ((Black) `(Node ,c ,l ,k ,r))))))))))
                    ((Black) `(Node ,c ,l ,k ,r))))))
          ((Node c0 a x c_)
            (match c0
               ((Red)
                 (match a
                    ((Leaf)
                      (match c_
                         ((Leaf)
                           (match r
                              ((Leaf) `(Node ,c ,l ,k ,r))
                              ((Node c1 b y d)
                                (match c1
                                   ((Red)
                                     (match b
                                        ((Leaf)
                                          (match d
                                             ((Leaf) `(Node ,c ,l ,k ,r))
                                             ((Node c2 c_0 z d0)
                                               (match c2
                                                  ((Red) `(Node ,`(Red)
                                                    ,`(Node ,`(Black) ,l ,k
                                                    ,b) ,y ,`(Node ,`(Black)
                                                    ,c_0 ,z ,d0)))
                                                  ((Black) `(Node ,c ,l ,k
                                                    ,r))))))
                                        ((Node c2 b0 y0 c_0)
                                          (match c2
                                             ((Red) `(Node ,`(Red) ,`(Node
                                               ,`(Black) ,l ,k ,b0) ,y0
                                               ,`(Node ,`(Black) ,c_0 ,y
                                               ,d)))
                                             ((Black)
                                               (match d
                                                  ((Leaf) `(Node ,c ,l ,k
                                                    ,r))
                                                  ((Node c3 c_1 z d0)
                                                    (match c3
                                                       ((Red) `(Node ,`(Red)
                                                         ,`(Node ,`(Black) ,l
                                                         ,k ,b) ,y ,`(Node
                                                         ,`(Black) ,c_1 ,z
                                                         ,d0)))
                                                       ((Black) `(Node ,c ,l
                                                         ,k ,r))))))))))
                                   ((Black) `(Node ,c ,l ,k ,r))))))
                         ((Node c1 b y c_0)
                           (match c1
                              ((Red) `(Node ,`(Red) ,`(Node ,`(Black) ,a ,x
                                ,b) ,y ,`(Node ,`(Black) ,c_0 ,k ,r)))
                              ((Black)
                                (match r
                                   ((Leaf) `(Node ,c ,l ,k ,r))
                                   ((Node c2 b0 y0 d)
                                     (match c2
                                        ((Red)
                                          (match b0
                                             ((Leaf)
                                               (match d
                                                  ((Leaf) `(Node ,c ,l ,k
                                                    ,r))
                                                  ((Node c3 c_1 z d0)
                                                    (match c3
                                                       ((Red) `(Node ,`(Red)
                                                         ,`(Node ,`(Black) ,l
                                                         ,k ,b0) ,y0 ,`(Node
                                                         ,`(Black) ,c_1 ,z
                                                         ,d0)))
                                                       ((Black) `(Node ,c ,l
                                                         ,k ,r))))))
                                             ((Node c3 b1 y1 c_1)
                                               (match c3
                                                  ((Red) `(Node ,`(Red)
                                                    ,`(Node ,`(Black) ,l ,k
                                                    ,b1) ,y1 ,`(Node
                                                    ,`(Black) ,c_1 ,y0 ,d)))
                                                  ((Black)
                                                    (match d
                                                       ((Leaf) `(Node ,c ,l
                                                         ,k ,r))
                                                       ((Node c4 c_2 z d0)
                                                         (match c4
                                                            ((Red) `(Node
                                                              ,`(Red) ,`(Node
                                                              ,`(Black) ,l ,k
                                                              ,b0) ,y0
                                                              ,`(Node
                                                              ,`(Black) ,c_2
                                                              ,z ,d0)))
                                                            ((Black) `(Node
                                                              ,c ,l ,k ,r))))))))))
                                        ((Black) `(Node ,c ,l ,k ,r))))))))))
                    ((Node c1 a0 x0 b)
                      (match c1
                         ((Red) `(Node ,`(Red) ,`(Node ,`(Black) ,a0 ,x0 ,b)
                           ,x ,`(Node ,`(Black) ,c_ ,k ,r)))
                         ((Black)
                           (match c_
                              ((Leaf)
                                (match r
                                   ((Leaf) `(Node ,c ,l ,k ,r))
                                   ((Node c2 b0 y d)
                                     (match c2
                                        ((Red)
                                          (match b0
                                             ((Leaf)
                                               (match d
                                                  ((Leaf) `(Node ,c ,l ,k
                                                    ,r))
                                                  ((Node c3 c_0 z d0)
                                                    (match c3
                                                       ((Red) `(Node ,`(Red)
                                                         ,`(Node ,`(Black) ,l
                                                         ,k ,b0) ,y ,`(Node
                                                         ,`(Black) ,c_0 ,z
                                                         ,d0)))
                                                       ((Black) `(Node ,c ,l
                                                         ,k ,r))))))
                                             ((Node c3 b1 y0 c_0)
                                               (match c3
                                                  ((Red) `(Node ,`(Red)
                                                    ,`(Node ,`(Black) ,l ,k
                                                    ,b1) ,y0 ,`(Node
                                                    ,`(Black) ,c_0 ,y ,d)))
                                                  ((Black)
                                                    (match d
                                                       ((Leaf) `(Node ,c ,l
                                                         ,k ,r))
                                                       ((Node c4 c_1 z d0)
                                                         (match c4
                                                            ((Red) `(Node
                                                              ,`(Red) ,`(Node
                                                              ,`(Black) ,l ,k
                                                              ,b0) ,y ,`(Node
                                                              ,`(Black) ,c_1
                                                              ,z ,d0)))
                                                            ((Black) `(Node
                                                              ,c ,l ,k ,r))))))))))
                                        ((Black) `(Node ,c ,l ,k ,r))))))
                              ((Node c2 b0 y c_0)
                                (match c2
                                   ((Red) `(Node ,`(Red) ,`(Node ,`(Black) ,a
                                     ,x ,b0) ,y ,`(Node ,`(Black) ,c_0 ,k
                                     ,r)))
                                   ((Black)
                                     (match r
                                        ((Leaf) `(Node ,c ,l ,k ,r))
                                        ((Node c3 b1 y0 d)
                                          (match c3
                                             ((Red)
                                               (match b1
                                                  ((Leaf)
                                                    (match d
                                                       ((Leaf) `(Node ,c ,l
                                                         ,k ,r))
                                                       ((Node c4 c_1 z d0)
                                                         (match c4
                                                            ((Red) `(Node
                                                              ,`(Red) ,`(Node
                                                              ,`(Black) ,l ,k
                                                              ,b1) ,y0
                                                              ,`(Node
                                                              ,`(Black) ,c_1
                                                              ,z ,d0)))
                                                            ((Black) `(Node
                                                              ,c ,l ,k ,r))))))
                                                  ((Node c4 b2 y1 c_1)
                                                    (match c4
                                                       ((Red) `(Node ,`(Red)
                                                         ,`(Node ,`(Black) ,l
                                                         ,k ,b2) ,y1 ,`(Node
                                                         ,`(Black) ,c_1 ,y0
                                                         ,d)))
                                                       ((Black)
                                                         (match d
                                                            ((Leaf) `(Node ,c
                                                              ,l ,k ,r))
                                                            ((Node c5 c_2 z
                                                              d0)
                                                              (match c5
                                                                 ((Red)
                                                                   `(Node
                                                                   ,`(Red)
                                                                   ,`(Node
                                                                   ,`(Black)
                                                                   ,l ,k ,b1)
                                                                   ,y0
                                                                   ,`(Node
                                                                   ,`(Black)
                                                                   ,c_2 ,z
                                                                   ,d0)))
                                                                 ((Black)
                                                                   `(Node ,c
                                                                   ,l ,k ,r))))))))))
                                             ((Black) `(Node ,c ,l ,k ,r))))))))))))))
               ((Black)
                 (match r
                    ((Leaf) `(Node ,c ,l ,k ,r))
                    ((Node c1 b y d)
                      (match c1
                         ((Red)
                           (match b
                              ((Leaf)
                                (match d
                                   ((Leaf) `(Node ,c ,l ,k ,r))
                                   ((Node c2 c_0 z d0)
                                     (match c2
                                        ((Red) `(Node ,`(Red) ,`(Node
                                          ,`(Black) ,l ,k ,b) ,y ,`(Node
                                          ,`(Black) ,c_0 ,z ,d0)))
                                        ((Black) `(Node ,c ,l ,k ,r))))))
                              ((Node c2 b0 y0 c_0)
                                (match c2
                                   ((Red) `(Node ,`(Red) ,`(Node ,`(Black) ,l
                                     ,k ,b0) ,y0 ,`(Node ,`(Black) ,c_0 ,y
                                     ,d)))
                                   ((Black)
                                     (match d
                                        ((Leaf) `(Node ,c ,l ,k ,r))
                                        ((Node c3 c_1 z d0)
                                          (match c3
                                             ((Red) `(Node ,`(Red) ,`(Node
                                               ,`(Black) ,l ,k ,b) ,y ,`(Node
                                               ,`(Black) ,c_1 ,z ,d0)))
                                             ((Black) `(Node ,c ,l ,k ,r))))))))))
                         ((Black) `(Node ,c ,l ,k ,r)))))))))))))

(define ins (lambdas (x t)
  (match t
     ((Leaf) `(Node ,`(Red) ,`(Leaf) ,x ,`(Leaf)))
     ((Node c l k r)
       (match (@ ltb x k)
          ((True) (@ balance c (@ ins x l) k r))
          ((False)
            (match (@ ltb k x)
               ((True) (@ balance c l k (@ ins x r)))
               ((False) t))))))))
  
(define make_black (lambda (t)
  (match t
     ((Leaf) `(Leaf))
     ((Node _ l k r) `(Node ,`(Black) ,l ,k ,r)))))

(define insert (lambdas (x t) (make_black (@ ins x t))))

(define insert_list (lambdas (xs t)
  (match xs
     ((Nil) t)
     ((Cons x rest) (@ insert_list rest (@ insert x t))))))
  
(define all_member (lambdas (xs t)
  (match xs
     ((Nil) `(True))
     ((Cons x rest)
       (match (@ member x t)
          ((True) (@ all_member rest t))
          ((False) `(False)))))))
  
(define depth (lambda (t)
  (match t
     ((Leaf) `(0))
     ((Node _ l _ r)
       (@ add `((lambda (n) (+ n 1)) ,`(0)) (@ max (depth l) (depth r)))))))
  
(define size (lambda (t)
  (match t
     ((Leaf) `(0))
     ((Node _ l _ r)
       (@ add (@ add `((lambda (n) (+ n 1)) ,`(0)) (size l)) (size r))))))
  
(define seq (lambdas (start len)
  ((lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))
     (lambda (_) `(Nil))
     (lambda (n) `(Cons ,start ,(@ seq `((lambda (n) (+ n 1)) ,start) n)))
     len)))
  
(define build_tree (lambda (n) (@ insert_list (@ seq `(0) n) `(Leaf))))

(define build_and_check (lambda (n)
  (let ((t (build_tree n))) (@ all_member (@ seq `(0) n) t))))

