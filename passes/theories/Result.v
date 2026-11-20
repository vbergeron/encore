(** Error monad shared by the IR codecs and the passes. *)

From Stdlib Require Import String List.
Import ListNotations.
Open Scope string_scope.

Inductive result (A : Type) : Type :=
| Ok (a : A)
| Err (msg : string).

Arguments Ok {A} a.
Arguments Err {A} msg.

Definition bind {A B} (m : result A) (k : A -> result B) : result B :=
  match m with
  | Ok a => k a
  | Err e => Err e
  end.

Definition fmap {A B} (f : A -> B) (m : result A) : result B :=
  bind m (fun a => Ok (f a)).

Notation "'let*' x := m 'in' k" := (bind m (fun x => k))
  (at level 200, x name, m at level 100, k at level 200).

(** [mapM] is written with [f] as a section variable so that the guard
    checker can see through it when a fixpoint over a nested inductive
    calls [mapM self children]. *)
Section MapM.
  Context {A B : Type} (f : A -> result B).

  Fixpoint mapM (l : list A) : result (list B) :=
    match l with
    | [] => Ok []
    | x :: xs =>
        let* y := f x in
        let* ys := mapM xs in
        Ok (y :: ys)
    end.
End MapM.
