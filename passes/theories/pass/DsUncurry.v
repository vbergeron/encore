(** [ds_uncurry]: uncurry: multi-argument lambdas, saturated applications
    (crates/encore_compiler/src/pass/ds_uncurry.rs).
    Produces stage [ds-uncurry]. *)

From Stdlib Require Import String.
From EncorePasses Require Import Result.
From EncorePasses Require Ds Dsi Cps Asm.
Open Scope string_scope.

Definition run (m : Ds.module) : result Ds.module :=
  Err "ds-uncurry is not implemented yet".
