; Counter FSM — demonstrates Rust/Encore interop
;
; State: integer counter
; Events: Inc | Dec | Reset
; Effects: Print(val) | Beep
;
; step : State -> Event -> Pair(State, List Effect)

(define init 0)

(define step (lambdas (state event)
  (match event
    ((Inc)
      (let ((s (+ state 1)))
        `(Pair ,s ,`(Cons ,`(Print ,s) ,`(Nil)))))
    ((Dec)
      (let ((s (- state 1)))
        (if (= s 0)
          `(Pair ,s ,`(Cons ,`(Print ,s) ,`(Cons ,`(Beep) ,`(Nil))))
          `(Pair ,s ,`(Cons ,`(Print ,s) ,`(Nil))))))
    ((Reset)
      `(Pair ,0 ,`(Cons ,`(Print ,0) ,`(Cons ,`(Beep) ,`(Nil))))))))
