(** [cps_optimize]: the CPS optimizer (crates/encore_compiler/src/pass/cps_optimize.rs,
    OPTIMIZER.md). Not implemented yet: the identity, which is what the Rust
    pipeline does with [--cps-optimize off].
    Produces stage [cps-optimize]. *)

From Stdlib Require Import String.
From EncorePasses Require Import Result.
From EncorePasses Require Ds Dsi Cps Asm.
Open Scope string_scope.

Definition run (m : Cps.module) : result Cps.module := Ok m.
