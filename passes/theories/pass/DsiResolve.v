(** [dsi_resolve]: resolve names to de Bruijn indices
    (crates/encore_compiler/src/pass/dsi_resolve.rs).
    Produces stage [dsi-resolve]. *)

From Stdlib Require Import String.
From EncorePasses Require Import Result.
From EncorePasses Require Ds Dsi Cps Asm.
Open Scope string_scope.

Definition run (m : Ds.module) : result Dsi.module :=
  Err "dsi-resolve is not implemented yet".
