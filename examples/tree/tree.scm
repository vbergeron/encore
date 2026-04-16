; Count leaves in a binary tree
; count(Node(Node(Leaf, Leaf), Node(Leaf, Node(Leaf, Leaf)))) = 5

(define count (lambda (t)
  (match t
    ((Leaf) 1)
    ((Node l r)
      (+ (count l) (count r))))))

(define main
  (count `(Node ,`(Node ,`(Leaf) ,`(Leaf))
               ,`(Node ,`(Leaf) ,`(Node ,`(Leaf) ,`(Leaf))))))
