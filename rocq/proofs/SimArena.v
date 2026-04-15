(** * Pure functional simulation of [encore_vm::arena::Arena].

    This module defines an idealized, purely functional model of the
    Arena and its operations.  Properties are stated and proved on this
    model directly, avoiding the need to reason about the [M] monad. *)

Require Import Stdlib.Lists.List.
Require Import Stdlib.ZArith.ZArith.
Require Import Stdlib.micromega.Lia.
Import ListNotations.
Open Scope Z_scope.

(* ----------------------------------------------------------------- *)
(** ** List helpers *)

Fixpoint replace_at {A : Type} (l : list A) (i : nat) (v : A) : list A :=
  match l with
  | [] => []
  | x :: l' =>
    match i with
    | O   => v :: l'
    | S j => x :: replace_at l' j v
    end
  end.

Lemma nth_replace_at_same {A : Type} (l : list A) (i : nat) (v d : A) :
  (i < length l)%nat ->
  nth i (replace_at l i v) d = v.
Proof.
  revert l; induction i; intros [|x l'] H; simpl in *; try lia.
  - reflexivity.
  - apply IHi. lia.
Qed.

Lemma nth_replace_at_other {A : Type} (l : list A) (i j : nat) (v d : A) :
  i <> j ->
  nth j (replace_at l i v) d = nth j l d.
Proof.
  revert l j; induction i; intros [|x l'] [|j'] Hneq; simpl; try reflexivity; try lia.
  - apply IHi. lia.
Qed.

(* ----------------------------------------------------------------- *)
(** ** Arena model *)

Module Arena.
  Record t := mk {
    hp  : Z;
    mem : list Z;
  }.
End Arena.

Definition arena_new (mem : list Z) : Arena.t :=
  Arena.mk 0 mem.

Definition arena_hp (a : Arena.t) : Z :=
  Arena.hp a.

Definition arena_heap_read (a : Arena.t) (addr off : Z) : Z :=
  nth (Z.to_nat (addr + off)) (Arena.mem a) 0.

Definition arena_heap_write (a : Arena.t) (addr off v : Z) : Arena.t :=
  Arena.mk (Arena.hp a) (replace_at (Arena.mem a) (Z.to_nat (addr + off)) v).

Definition arena_alloc (a : Arena.t) (n : Z) : option (Z * Arena.t) :=
  if (Arena.hp a + n <=? Z.of_nat (length (Arena.mem a)))%Z
  then Some (Arena.hp a, Arena.mk (Arena.hp a + n) (Arena.mem a))
  else None.

(* ----------------------------------------------------------------- *)
(** ** Properties *)

(** [arena_new] initialises [hp] to 0. *)
Theorem new_hp_zero : forall mem,
  arena_hp (arena_new mem) = 0.
Proof. reflexivity. Qed.

(** [arena_hp] returns the [hp] field. *)
Theorem hp_returns_field : forall a,
  arena_hp a = Arena.hp a.
Proof. reflexivity. Qed.

(** When [arena_alloc] succeeds, the returned address is the old [hp]. *)
Theorem alloc_returns_old_hp : forall a n addr a',
  arena_alloc a n = Some (addr, a') ->
  addr = arena_hp a.
Proof.
  intros a n addr a' H.
  unfold arena_alloc in H.
  destruct (Arena.hp a + n <=? Z.of_nat (length (Arena.mem a)))%Z;
    inversion H.
  reflexivity.
Qed.

(** [heap_write] then [heap_read] at the same location returns the
    written value. *)
Theorem heap_write_read_same : forall a addr off v,
  (Z.to_nat (addr + off) < length (Arena.mem a))%nat ->
  arena_heap_read (arena_heap_write a addr off v) addr off = v.
Proof.
  intros a addr off v Hbound.
  unfold arena_heap_read, arena_heap_write; simpl.
  apply nth_replace_at_same.
  exact Hbound.
Qed.

(** [heap_write] does not affect [heap_read] at a different index. *)
Theorem heap_write_read_other : forall a addr1 off1 addr2 off2 v,
  Z.to_nat (addr1 + off1) <> Z.to_nat (addr2 + off2) ->
  arena_heap_read (arena_heap_write a addr1 off1 v) addr2 off2 =
  arena_heap_read a addr2 off2.
Proof.
  intros a addr1 off1 addr2 off2 v Hneq.
  unfold arena_heap_read, arena_heap_write; simpl.
  apply nth_replace_at_other.
  exact Hneq.
Qed.

(** [heap_write] preserves the heap pointer. *)
Theorem heap_write_preserves_hp : forall a addr off v,
  arena_hp (arena_heap_write a addr off v) = arena_hp a.
Proof. reflexivity. Qed.
