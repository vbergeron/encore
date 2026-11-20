(** [cps_transform]: make continuations explicit
    (crates/encore_compiler/src/pass/cps_transform.rs).
    Produces stage [cps-transform]. *)

From Stdlib Require Import String.
From EncorePasses Require Import Result.
From EncorePasses Require Ds Dsi Cps Asm.
Open Scope string_scope.

Definition run (m : Dsi.module) : result Cps.module :=
  Err "cps-transform is not implemented yet".
