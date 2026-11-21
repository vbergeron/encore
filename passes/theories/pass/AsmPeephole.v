(** [asm_peephole]: peephole rewrites on registers
    (crates/encore_compiler/src/pass/asm_peephole.rs). Not implemented yet:
    the identity, which is always a valid result.
    Produces stage [asm-peephole]. *)

From Stdlib Require Import String.
From EncorePasses Require Import Result.
From EncorePasses Require Ds Dsi Cps Asm.
From EncorePasses Require Import Runtime Correctness.
Open Scope string_scope.

Definition run (m : Asm.module) : result Asm.module := Ok m.

(** The identity preserves the behaviour of every program, whatever the
    semantics of [Asm] turns out to be. *)
Lemma run_preserves (sem : Asm.module -> nat -> load_obs -> Prop) : preserves sem sem run.
Proof. apply correct_id, load_refines_refl. Qed.
