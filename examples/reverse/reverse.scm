; Build, reverse, then measure length of a list
; length(reverse(make(100))) = 100

(define make (lambda (n)
  (if (= n 0)
    `(Nil)
    `(Cons ,n ,(make (- n 1))))))

(define rev (lambda (p)
  (match p
    ((Pair lst acc)
      (match lst
        ((Nil) acc)
        ((Cons h t) (rev `(Pair ,t ,`(Cons ,h ,acc)))))))))

(define length (lambda (lst)
  (match lst
    ((Nil) 0)
    ((Cons h t) (+ 1 (length t))))))

(define main
  (let ((l (make 100)))
    (length (rev `(Pair ,l ,`(Nil))))))
