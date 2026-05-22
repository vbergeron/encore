From Stdlib Require Import Arith.
Require Import Extraction.

(** Opaque wrappers so [Extract Constant] can replace them with VM primitives.
    Rocq's [Nat.add] / [Nat.sub] are fixpoints and cannot be targeted directly. *)
Definition nat_add (n m : nat) : nat := n + m.
Definition nat_sub (n m : nat) : nat := n - m.

(** Subtraction-based Euclidean GCD with fuel.
    Each call reduces [|a - b|], so [fuel = nat_add a b] suffices.
    Uses only equality, comparison, and subtraction — all encore_vm primitives. *)
Fixpoint gcd_aux (fuel a b : nat) : nat :=
  match fuel with
  | O => a  (* fuel = a + b suffices; this case is unreachable *)
  | S fuel' =>
    if Nat.eqb a b then a
    else if Nat.leb b a
         then gcd_aux fuel' (nat_sub a b) b
         else gcd_aux fuel' a (nat_sub b a)
  end.

Definition gcd (a b : nat) : nat := gcd_aux (nat_add a b) a b.

(** Expose [gcd] as the entry point so it receives machine integers
    from the embedding (Rust) rather than Peano-encoded literals. *)
Definition main := gcd.

(** Extraction to encore's Scheme frontend.
    Natural numbers become machine integers; [nat] pattern matching uses
    the [(lambdas (fO fS n) ...)] eliminator understood by encore_scheme.
    The opaque wrappers [nat_add]/[nat_sub] are replaced with VM primitives. *)
Extract Inductive nat => "integer"
  ["0" "(lambda (x) (+ x 1))"]
  "(lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))".

Extract Constant nat_add => "(lambda (n) (lambda (m) (+ n m)))".
Extract Constant nat_sub => "(lambda (n) (lambda (m) (- n m)))".

Extract Constant Nat.eqb =>
  "(lambda (a) (lambda (b) (if (= a b) `(True) `(False))))".

Extract Constant Nat.leb =>
  "(lambda (a) (lambda (b) (if (< b a) `(False) `(True))))".

Extraction Language Scheme.
Extraction "gcd.scm" main.
