(** [asm_resolve]: allocate registers, captures and globals
    (crates/encore_compiler/src/pass/asm_resolve.rs).
    Produces stage [asm-resolve]. *)

From Stdlib Require Import String.
From EncorePasses Require Import Result.
From EncorePasses Require Ds Dsi Cps Asm.
Open Scope string_scope.

Definition run (m : Cps.module) : result Asm.module :=
  Err "asm-resolve is not implemented yet".
