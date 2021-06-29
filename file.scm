(define (node left right)
    (cons left right)
)
(define (make-tree depth) 
    (
        (if (<= depth 0)
            (cons #nil #nil)
            (cons (make-tree (- depth 1)) (make-tree (- depth 1)) ))
        )
    )


(make-tree 14)