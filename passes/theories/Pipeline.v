(** The pass pipeline, mirroring crates/encore_compiler/src/pipeline.rs.

    A stage is named after the pass that produced it. [compile from to]
    decodes text that is the output of [from], runs every pass after [from]
    up to and including [to], and encodes the result. *)

From Stdlib Require Import String List.
From EncorePasses Require Import Result Sexp.
From EncorePasses Require Ds Dsi Cps Asm.
From EncorePasses Require DsUncurry DsiResolve CpsTransform CpsOptimize AsmResolve AsmPeephole.
Import ListNotations.
Open Scope string_scope.

Inductive stage : Type :=
| Parse          (** ds, from the frontend (Fleche / Scheme) *)
| SDsUncurry     (** ds *)
| SDsiResolve    (** dsi *)
| SCpsTransform  (** cps *)
| SCpsOptimize   (** cps *)
| SAsmResolve    (** asm *)
| SAsmPeephole.  (** asm *)

(** In pipeline order. *)
Definition stages : list stage :=
  [Parse; SDsUncurry; SDsiResolve; SCpsTransform; SCpsOptimize; SAsmResolve; SAsmPeephole].

Definition stage_name (s : stage) : string :=
  match s with
  | Parse => "parse"
  | SDsUncurry => "ds-uncurry"
  | SDsiResolve => "dsi-resolve"
  | SCpsTransform => "cps-transform"
  | SCpsOptimize => "cps-optimize"
  | SAsmResolve => "asm-resolve"
  | SAsmPeephole => "asm-peephole"
  end.

Definition stage_names : list string := map stage_name stages.

Definition stage_of_name (n : string) : option stage :=
  find (fun s => String.eqb (stage_name s) n) stages.

Definition stage_index (s : stage) : nat :=
  match s with
  | Parse => 0 | SDsUncurry => 1 | SDsiResolve => 2 | SCpsTransform => 3
  | SCpsOptimize => 4 | SAsmResolve => 5 | SAsmPeephole => 6
  end.

(** A module of any IR. *)
Inductive ir : Type :=
| IDs (m : Ds.module)
| IDsi (m : Dsi.module)
| ICps (m : Cps.module)
| IAsm (m : Asm.module).

Inductive ir_kind : Type := KDs | KDsi | KCps | KAsm.

Definition stage_kind (s : stage) : ir_kind :=
  match s with
  | Parse | SDsUncurry => KDs
  | SDsiResolve => KDsi
  | SCpsTransform | SCpsOptimize => KCps
  | SAsmResolve | SAsmPeephole => KAsm
  end.

(** ** Text form: [header] is the leading [; ...] comment *)

Definition decode (s : stage) (forms : list sexp) : result ir :=
  match stage_kind s with
  | KDs => fmap IDs (Ds.decode forms)
  | KDsi => fmap IDsi (Dsi.decode forms)
  | KCps => fmap ICps (Cps.decode forms)
  | KAsm => fmap IAsm (Asm.decode forms)
  end.

Definition encode (m : ir) : string * list sexp :=
  match m with
  | IDs m => (Ds.header, Ds.encode m)
  | IDsi m => (Dsi.header, Dsi.encode m)
  | ICps m => (Cps.header, Cps.encode m)
  | IAsm m => (Asm.header, Asm.encode m)
  end.

(** ** Passes *)

Definition ill_typed {A} (s : stage) : result A :=
  Err ("internal error: wrong IR given to the pass producing " ++ stage_name s).

(** Run the pass that produces [s]. *)
Definition step (s : stage) (m : ir) : result ir :=
  match s, m with
  | Parse, _ => Err "parse is the frontend: there is no pass producing it"
  | SDsUncurry, IDs m => fmap IDs (DsUncurry.run m)
  | SDsiResolve, IDs m => fmap IDsi (DsiResolve.run m)
  | SCpsTransform, IDsi m => fmap ICps (CpsTransform.run m)
  | SCpsOptimize, ICps m => fmap ICps (CpsOptimize.run m)
  | SAsmResolve, ICps m => fmap IAsm (AsmResolve.run m)
  | SAsmPeephole, IAsm m => fmap IAsm (AsmPeephole.run m)
  | _, _ => ill_typed s
  end.

Fixpoint steps (ss : list stage) (m : ir) : result ir :=
  match ss with
  | [] => Ok m
  | s :: ss => let* m := step s m in steps ss m
  end.

(** The stages strictly after [from], up to and including [to]. *)
Definition between (from to : stage) : list stage :=
  filter (fun s => andb (Nat.ltb (stage_index from) (stage_index s))
                        (Nat.leb (stage_index s) (stage_index to))) stages.

Definition run (from to : stage) (m : ir) : result ir :=
  if Nat.ltb (stage_index to) (stage_index from)
  then Err ("cannot go back from " ++ stage_name from ++ " to " ++ stage_name to)
  else steps (between from to) m.

(** The entry point used by the OCaml driver. *)
Definition compile (from to : stage) (forms : list sexp) : result (string * list sexp) :=
  let* m := decode from forms in
  let* m := run from to m in
  Ok (encode m).
