From Stdlib Require Import Arith.

(** Subtraction-based Euclidean GCD with fuel.
    Each call reduces [|a - b|], so [fuel = a + b] suffices.
    Uses only equality, comparison, and subtraction, which [ExtrEncore]
    maps to encore_vm primitives. *)
Fixpoint gcd_aux (fuel a b : nat) : nat :=
  match fuel with
  | O => a  (* fuel = a + b suffices; this case is unreachable *)
  | S fuel' =>
    if a =? b then a
    else if b <=? a
         then gcd_aux fuel' (a - b) b
         else gcd_aux fuel' a (b - a)
  end.

Definition gcd (a b : nat) : nat := gcd_aux (a + b) a b.

(** Expose [gcd] as the entry point so it receives machine integers
    from the embedding (Rust) rather than Peano-encoded literals. *)
Definition main := gcd.

(** A closed test value, for [encore run --entry check] in CI. *)
Definition check := gcd 48 18.

Example check_ok : check = 6.
Proof. reflexivity. Qed.
