(** [asm_peephole]: peephole rewrites on registers
    (crates/encore_compiler/src/pass/asm_peephole.rs). Not implemented yet:
    the identity, which is always a valid result.
    Produces stage [asm-peephole]. *)

From Stdlib Require Import String.
From EncorePasses Require Import Result.
From EncorePasses Require Ds Dsi Cps Asm.
Open Scope string_scope.

Definition run (m : Asm.module) : result Asm.module := Ok m.
