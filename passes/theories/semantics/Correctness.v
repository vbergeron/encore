(** Correctness of a program transformation, relative to the semantics of
  its source and target languages.

  A semantics is a relation [sem p i o]: program [p], run on input [i],
  can be observed as [o]. A transformation is a partial function
  [t : P1 -> result P2], the shape of every pass in theories/pass.

  Correctness compares observable behaviour, as in CompCert, not machine
  states: a transformation changes code positions, register assignment,
  allocation and the number of steps, so the states of a program and of
  its transformed version are not equal. How states correspond during a
  run is part of each pass's proof (a simulation relation), not of the
  statement. *)

From EncorePasses Require Import Result.
From EncorePasses Require Import Runtime.

(** [t] is correct when every behaviour of a source program is matched,
    after a successful transformation, by a behaviour of the target that
    [refines] it, on the same input. This is a forward simulation: it
    constrains only source programs that have a behaviour. Once the
    target semantics is shown deterministic (up to renaming of heap
    addresses, for the machine-level ones), the target behaviour it
    provides is the target's only one. A transformation that fails ([Err]) is trivially correct: the
    pipeline reports the error instead of producing code. *)
Definition correct {P1 P2 I O1 O2 : Type} (sem1 : P1 -> I -> O1 -> Prop) (sem2 : P2 -> I -> O2 -> Prop)
    (refines : O1 -> O2 -> Prop) (t : P1 -> result P2) : Prop :=
  forall p1 p2, t p1 = Ok p2 ->
  forall i o1, sem1 p1 i o1 -> exists o2, sem2 p2 i o2 /\ refines o1 o2.

(** Running [t1] then [t2], as the pipeline does. *)
Definition compose {P1 P2 P3 : Type} (t1 : P1 -> result P2) (t2 : P2 -> result P3) (p : P1) : result P3 :=
  let* q := t1 p in t2 q.

Lemma correct_id {P I O : Type} (sem : P -> I -> O -> Prop) (refines : O -> O -> Prop) :
  (forall o, refines o o) -> correct sem sem refines (fun p => Ok p).
Proof.
  intros Hrefl p1 p2 Ht i o1 Hsem. injection Ht as <-. exists o1. auto.
Qed.

Lemma correct_compose {P1 P2 P3 I O1 O2 O3 : Type}
    (sem1 : P1 -> I -> O1 -> Prop) (sem2 : P2 -> I -> O2 -> Prop) (sem3 : P3 -> I -> O3 -> Prop)
    (r12 : O1 -> O2 -> Prop) (r23 : O2 -> O3 -> Prop) (r13 : O1 -> O3 -> Prop)
    (t1 : P1 -> result P2) (t2 : P2 -> result P3) :
  (forall o1 o2 o3, r12 o1 o2 -> r23 o2 o3 -> r13 o1 o3) ->
  correct sem1 sem2 r12 t1 -> correct sem2 sem3 r23 t2 ->
  correct sem1 sem3 r13 (compose t1 t2).
Proof.
  intros Htrans H1 H2 p1 p3 Ht i o1 Hsem1. unfold compose, bind in Ht.
  destruct (t1 p1) as [p2 |] eqn:E1; [| discriminate].
  destruct (H1 p1 p2 E1 i o1 Hsem1) as (o2 & Hsem2 & R12).
  destruct (H2 p2 p3 Ht i o2 Hsem2) as (o3 & Hsem3 & R23).
  eauto.
Qed.

(** For Encore's machine-level languages, programs are observed through
  [load] ([load_obs]) and a target may only differ by running out of
  memory ([load_refines]). The input is the size of the memory buffer. *)
Definition preserves {P1 P2 : Type} (sem1 : P1 -> nat -> load_obs -> Prop)
  (sem2 : P2 -> nat -> load_obs -> Prop) (t : P1 -> result P2) : Prop :=
correct sem1 sem2 load_refines t.

Lemma preserves_compose {P1 P2 P3 : Type}
  (sem1 : P1 -> nat -> load_obs -> Prop) (sem2 : P2 -> nat -> load_obs -> Prop)
  (sem3 : P3 -> nat -> load_obs -> Prop) (t1 : P1 -> result P2) (t2 : P2 -> result P3) :
preserves sem1 sem2 t1 -> preserves sem2 sem3 t2 -> preserves sem1 sem3 (compose t1 t2).
Proof. apply correct_compose, load_refines_trans. Qed.
