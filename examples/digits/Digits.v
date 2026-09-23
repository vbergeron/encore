From Stdlib Require Import Arith List String.
Import ListNotations.
From Encore.Extraction Require Import ExtrEncoreBytes.

(** Decimal rendering of a [nat] as a byte string.

    Exercises the parts of [ExtrEncore] that [gcd] does not: [Nat.div] and
    [Nat.modulo] (VM division primitives), [list] and [bool] (pre-registered
    constructors), and [string] literals as byte strings. *)

(** Decimal digits of [n], most significant first. [fuel] bounds the number
    of digits; [n] itself is always enough. *)
Fixpoint digits_aux (fuel n : nat) (acc : list nat) : list nat :=
  match fuel with
  | O => acc
  | S fuel' =>
    if n <? 10 then n :: acc
    else digits_aux fuel' (n / 10) (n mod 10 :: acc)
  end.

Definition digits (n : nat) : list nat := digits_aux n n [].

(** Each digit is below 10, so [48 + d] is an ASCII digit and in range
    for [byte_of_nat]. *)
Definition render (n : nat) : bytes :=
  fold_left (fun acc d => bytes_concat acc (byte_of_nat (48 + d)))
            (digits n) (bytes_of_string "n="%string).

Definition main := render.

(** A closed test value, for [encore run --entry check] in CI: the length
    of the rendering if it is the expected text, [0] otherwise. *)
Definition check : nat :=
  let r := render 907 in
  if bytes_eqb r (bytes_of_string "n=907"%string) then bytes_len r else 0.

Example digits_ok : digits 907 = [9; 0; 7].
Proof. reflexivity. Qed.
