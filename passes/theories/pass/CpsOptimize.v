(** [cps_optimize]: the CPS optimizer (crates/encore_compiler/src/pass/cps_optimize.rs,
    OPTIMIZER.md). Not implemented yet: the identity, which is what the Rust
    pipeline does with [--cps-optimize off].
    Produces stage [cps-optimize]. *)

From Stdlib Require Import String.
From EncorePasses Require Import Result.
From EncorePasses Require Ds Dsi Cps Asm.
From EncorePasses Require Import Runtime Correctness.
Open Scope string_scope.

Definition run (m : Cps.module) : result Cps.module := Ok m.

(** The identity preserves the behaviour of every program, whatever the
    semantics of [Cps] turns out to be. *)
Lemma run_preserves (sem : Cps.module -> nat -> load_obs -> Prop) : preserves sem sem run.
Proof. apply correct_id, load_refines_refl. Qed.
