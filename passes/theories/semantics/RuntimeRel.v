(** Runtime operations respect [word_rel]: two machines whose registers,
    globals and heaps are equal up to code addresses related by [rho]
    find the same objects, keep the same objects alive, allocate the same
    way and observe the same results. The simulation proofs of the passes
    rely on these lemmas. *)

From Stdlib Require Import List PeanoNat.
Import ListNotations.
From EncorePasses Require Import Runtime.

(** ** Lists *)

Lemma Forall2_In_l {A B} (P : A -> B -> Prop) l1 l2 x :
  Forall2 P l1 l2 -> In x l1 -> exists y, In y l2 /\ P x y.
Proof.
  induction 1 as [| a b l1 l2 Hab _ IH]; simpl; [tauto |].
  intros [-> | Hin]; [eauto |]. destruct (IH Hin) as (y & ? & ?); eauto.
Qed.

Lemma Forall2_flip {A B} (P : A -> B -> Prop) l1 l2 :
  Forall2 P l1 l2 -> Forall2 (fun y x => P x y) l2 l1.
Proof. induction 1; auto. Qed.

Lemma Forall2_impl {A B} (P Q : A -> B -> Prop) l1 l2 :
  (forall x y, P x y -> Q x y) -> Forall2 P l1 l2 -> Forall2 Q l1 l2.
Proof. intros HPQ; induction 1; auto. Qed.

Lemma Forall2_map {A B C} (P : B -> C -> Prop) (f : A -> B) (g : A -> C) l :
  (forall x, In x l -> P (f x) (g x)) -> Forall2 P (map f l) (map g l).
Proof. induction l as [| x l IH]; simpl; auto with datatypes. Qed.

Lemma Forall2_nth_error {A B} (P : A -> B -> Prop) l1 l2 i x :
  Forall2 P l1 l2 -> nth_error l1 i = Some x -> exists y, nth_error l2 i = Some y /\ P x y.
Proof.
  intros H; revert i; induction H as [| a b l1 l2 Hab _ IH]; intros [| i]; simpl; try discriminate.
  - intros [= <-]; eauto.
  - apply IH.
Qed.

Lemma Forall2_nth_error_none {A B} (P : A -> B -> Prop) l1 l2 i :
  Forall2 P l1 l2 -> nth_error l1 i = None -> nth_error l2 i = None.
Proof.
  intros H; revert i; induction H; intros [| i]; simpl; auto; discriminate.
Qed.

Section Rel.
  Context {L1 L2 : Type} (rho : L1 -> L2 -> Prop).

  Lemma size_rel o1 o2 : obj_rel rho o1 o2 -> size o1 = size o2.
  Proof.
    destruct 1; simpl; auto; f_equal; f_equal; eapply Forall2_length; eauto.
  Qed.

  (** [hget] finds related objects, or nothing on both sides. *)
  Definition option_obj_rel (x : option (obj L1)) (y : option (obj L2)) : Prop :=
    match x, y with
    | Some o1, Some o2 => obj_rel rho o1 o2
    | None, None => True
    | _, _ => False
    end.

  Lemma hget_rel H1 H2 a : heap_rel rho H1 H2 -> option_obj_rel (hget H1 a) (hget H2 a).
  Proof.
    induction 1 as [| [b1 o1] [b2 o2] H1 H2 [Hb Ho] _ IH]; simpl in *; auto.
    subst. destruct (Nat.eqb a b2); auto.
  Qed.

  Lemma hget_rel_some H1 H2 a o1 :
    heap_rel rho H1 H2 -> hget H1 a = Some o1 -> exists o2, hget H2 a = Some o2 /\ obj_rel rho o1 o2.
  Proof.
    intros HH Hg. pose proof (hget_rel _ _ a HH) as R. rewrite Hg in R.
    destruct (hget H2 a); [eauto | contradiction].
  Qed.

  Lemma hget_rel_none H1 H2 a : heap_rel rho H1 H2 -> hget H1 a = None -> hget H2 a = None.
  Proof.
    intros HH Hg. pose proof (hget_rel _ _ a HH) as R. rewrite Hg in R.
    destruct (hget H2 a); [contradiction | reflexivity].
  Qed.

  Lemma live_size_rel H1 H2 : heap_rel rho H1 H2 -> live_size H1 = live_size H2.
  Proof.
    induction 1 as [| [b1 o1] [b2 o2] H1 H2 [_ Ho] _ IH]; simpl in *; auto.
    rewrite (size_rel _ _ Ho), IH. reflexivity.
  Qed.

  Lemma points_rel w1 w2 a : word_rel rho w1 w2 -> points w1 a <-> points w2 a.
  Proof. destruct 1; simpl; tauto. Qed.
End Rel.

(** ** Flipping the relation *)

Definition flip_rel {L1 L2} (rho : L1 -> L2 -> Prop) : L2 -> L1 -> Prop := fun y x => rho x y.

Lemma word_rel_flip {L1 L2} (rho : L1 -> L2 -> Prop) w1 w2 :
  word_rel rho w1 w2 -> word_rel (flip_rel rho) w2 w1.
Proof. destruct 1; constructor; auto. Qed.

Lemma obj_rel_flip {L1 L2} (rho : L1 -> L2 -> Prop) o1 o2 :
  obj_rel rho o1 o2 -> obj_rel (flip_rel rho) o2 o1.
Proof.
  destruct 1; constructor; auto;
    (eapply Forall2_impl; [| apply Forall2_flip; eassumption]; intros; apply word_rel_flip; auto).
Qed.

Lemma heap_rel_flip {L1 L2} (rho : L1 -> L2 -> Prop) H1 H2 :
  heap_rel rho H1 H2 -> heap_rel (flip_rel rho) H2 H1.
Proof.
  unfold heap_rel. intros HH. eapply Forall2_impl; [| apply Forall2_flip; exact HH].
  intros p2 p1 [Hf Ho]; split; [auto | apply obj_rel_flip; auto].
Qed.

Lemma regs_rel_flip {L1 L2} (rho : L1 -> L2 -> Prop) R1 R2 :
  regs_rel rho R1 R2 -> regs_rel (flip_rel rho) R2 R1.
Proof. intros HR r. apply word_rel_flip, HR. Qed.

Lemma words_rel_flip {L1 L2} (rho : L1 -> L2 -> Prop) G1 G2 :
  Forall2 (word_rel rho) G1 G2 -> Forall2 (word_rel (flip_rel rho)) G2 G1.
Proof.
  intros HG. eapply Forall2_impl; [| apply Forall2_flip; exact HG]. intros; apply word_rel_flip; auto.
Qed.

(** ** Reachability *)

Lemma obj_words_rel {L1 L2} (rho : L1 -> L2 -> Prop) o1 o2 :
  obj_rel rho o1 o2 -> Forall2 (word_rel rho) (obj_words o1) (obj_words o2).
Proof. destruct 1; simpl; auto. Qed.
Lemma reachable_rel {L1 L2} (rho : L1 -> L2 -> Prop) R1 R2 G1 G2 H1 H2 a :
  regs_rel rho R1 R2 -> Forall2 (word_rel rho) G1 G2 -> heap_rel rho H1 H2 ->
  reachable R1 G1 H1 a -> reachable R2 G2 H2 a.
Proof.
  intros HR HG HH. induction 1 as [r a Hr Hp | w a Hin Hp | a o w b _ IH Hg Hin Hp].
  - eapply reach_reg; [eassumption |]. exact (proj1 (points_rel rho _ _ a (HR r)) Hp).
  - destruct (Forall2_In_l _ _ _ _ HG Hin) as (w2 & Hin2 & Hw).
    eapply reach_global; [exact Hin2 |]. exact (proj1 (points_rel rho _ _ a Hw) Hp).
  - destruct (hget_rel_some rho _ _ _ _ HH Hg) as (o2 & Hg2 & Ho).
    destruct (Forall2_In_l _ _ _ _ (obj_words_rel rho _ _ Ho) Hin) as (w2 & Hin2 & Hw).
    eapply reach_heap; [exact IH | exact Hg2 | exact Hin2 |]. exact (proj1 (points_rel rho _ _ b Hw) Hp).
Qed.
Lemma reachable_rel_back {L1 L2} (rho : L1 -> L2 -> Prop) R1 R2 G1 G2 H1 H2 a :
  regs_rel rho R1 R2 -> Forall2 (word_rel rho) G1 G2 -> heap_rel rho H1 H2 ->
  reachable R2 G2 H2 a -> reachable R1 G1 H1 a.
Proof.
  intros HR HG HH. apply reachable_rel with (rho := flip_rel rho).
  - apply regs_rel_flip; auto.
  - apply words_rel_flip; auto.
  - apply heap_rel_flip; auto.
Qed.

(** ** Collection *)

Lemma hget_In {L} (H : heap L) a o : hget H a = Some o -> In (a, o) H.
Proof.
  induction H as [| [b o'] H IH]; simpl; [discriminate |].
  destruct (Nat.eqb_spec a b) as [-> | _]; [intros [= ->]; auto | auto].
Qed.

Lemma In_hget {L} (H : heap L) a o : NoDup (map fst H) -> In (a, o) H -> hget H a = Some o.
Proof.
  induction H as [| [b o'] H IH]; simpl; [contradiction |].
  intros ND [Heq | Hin]; inversion ND as [| ? ? Hnot ND']; subst.
  - inversion Heq; subst. rewrite Nat.eqb_refl. reflexivity.
  - destruct (Nat.eqb_spec a b) as [-> | _]; [| auto].
    exfalso. apply Hnot. change b with (fst (b, o)). apply in_map. exact Hin.
Qed.

Lemma heap_rel_fst {L1 L2} (rho : L1 -> L2 -> Prop) H1 H2 :
  heap_rel rho H1 H2 -> map fst H1 = map fst H2.
Proof. induction 1 as [| p1 p2 H1 H2 [Hf _] _ IH]; simpl; congruence. Qed.

Definition pick {L1 L2} (H2 : heap L2) (H1' : heap L1) : heap L2 :=
  flat_map (fun p => match hget H2 (fst p) with Some o => [(fst p, o)] | None => [] end) H1'.

Lemma pick_rel {L1 L2} (rho : L1 -> L2 -> Prop) (H2 : heap L2) (H1' : heap L1) :
  (forall p, In p H1' -> exists o2, hget H2 (fst p) = Some o2 /\ obj_rel rho (snd p) o2) ->
  heap_rel rho H1' (pick H2 H1').
Proof.
  induction H1' as [| p H1' IH]; simpl; intros Hp; [constructor |].
  destruct (Hp p (or_introl eq_refl)) as (o2 & Hg & Ho). rewrite Hg. simpl.
  constructor; [split; auto | apply IH; intros q Hq; apply Hp; auto].
Qed.

Lemma pick_from {L1 L2} (H2 : heap L2) (H1' : heap L1) a o :
  In (a, o) (pick H2 H1') -> hget H2 a = Some o.
Proof.
  unfold pick. rewrite in_flat_map. intros (p & _ & Hin).
  destruct (hget H2 (fst p)) eqn:E; simpl in Hin; [| contradiction].
  destruct Hin as [[= <- <-] | []]. exact E.
Qed.
Lemma collect_rel {L1 L2} (rho : L1 -> L2 -> Prop) R1 R2 G1 G2 H1 H2 H1' :
  regs_rel rho R1 R2 -> Forall2 (word_rel rho) G1 G2 -> heap_rel rho H1 H2 ->
  collect R1 G1 H1 H1' -> exists H2', collect R2 G2 H2 H2' /\ heap_rel rho H1' H2'.
Proof.
  intros HR HG HH [ND1 Hc1].
  assert (Hrel : heap_rel rho H1' (pick H2 H1')).
  { apply pick_rel. intros [a o1] Hin. simpl.
    apply (In_hget _ _ _ ND1) in Hin. apply Hc1 in Hin as [Hg _].
    exact (hget_rel_some rho _ _ _ _ HH Hg). }
  exists (pick H2 H1'). split; [split |]; auto.
  - rewrite <- (heap_rel_fst _ _ _ Hrel). exact ND1.
  - intros a o2. split.
    + intros Hg2. split; [exact (pick_from _ _ _ _ (hget_In _ _ _ Hg2)) |].
      destruct (hget_rel_some (flip_rel rho) _ _ _ _ (heap_rel_flip _ _ _ Hrel) Hg2) as (o1 & Hg1 & _).
      apply Hc1 in Hg1 as [_ Hr1]. exact (reachable_rel rho _ _ _ _ _ _ a HR HG HH Hr1).
    + intros [Hg2 Hr2].
      destruct (hget_rel_some (flip_rel rho) _ _ _ _ (heap_rel_flip _ _ _ HH) Hg2) as (o1 & Hg1 & _).
      assert (Hg1' : hget H1' a = Some o1).
      { apply Hc1. split; [exact Hg1 | exact (reachable_rel_back rho _ _ _ _ _ _ a HR HG HH Hr2)]. }
      destruct (hget_rel_some rho _ _ _ _ Hrel Hg1') as (o2' & Hg2' & _).
      pose proof (pick_from _ _ _ _ (hget_In _ _ _ Hg2')) as E. rewrite Hg2 in E. inversion E; subst.
      exact Hg2'.
Qed.

(** ** Allocation *)

Inductive alloc_result_rel {L1 L2} (rho : L1 -> L2 -> Prop) : alloc_result L1 -> alloc_result L2 -> Prop :=
| arr_ok H1 H2 hp a : heap_rel rho H1 H2 -> alloc_result_rel rho (Allocated H1 hp a) (Allocated H2 hp a)
| arr_oom H1 H2 hp : heap_rel rho H1 H2 -> alloc_result_rel rho (OutOfMemory H1 hp) (OutOfMemory H2 hp).

Lemma alloc_rel {L1 L2} (rho : L1 -> L2 -> Prop) cap R1 R2 G1 G2 H1 H2 hp o1 o2 res1 :
  regs_rel rho R1 R2 -> Forall2 (word_rel rho) G1 G2 -> heap_rel rho H1 H2 -> obj_rel rho o1 o2 ->
  alloc cap R1 G1 H1 hp o1 res1 ->
  exists res2, alloc cap R2 G2 H2 hp o2 res2 /\ alloc_result_rel rho res1 res2.
Proof.
  intros HR HG HH Ho Ha. pose proof (size_rel _ _ _ Ho) as Hs.
  destruct Ha as [a Hfit Hfresh | H1' a Hfull Hc Hfit Hfresh | H1' Hfull Hc Hover].
  - exists (Allocated ((a, o2) :: H2) (hp + size o1) a). split.
    + rewrite Hs. apply alloc_fast; [rewrite <- Hs; exact Hfit | exact (hget_rel_none _ _ _ _ HH Hfresh)].
    + constructor. constructor; [split; auto | exact HH].
  - destruct (collect_rel rho _ _ _ _ _ _ _ HR HG HH Hc) as (H2' & Hc2 & HH').
    exists (Allocated ((a, o2) :: H2') (live_size H1' + size o1) a). split.
    + rewrite Hs, (live_size_rel _ _ _ HH'). apply alloc_after_gc.
      * rewrite <- Hs; exact Hfull.
      * exact Hc2.
      * rewrite <- Hs, <- (live_size_rel _ _ _ HH'); exact Hfit.
      * exact (hget_rel_none _ _ _ _ HH' Hfresh).
    + constructor. constructor; [split; auto | exact HH'].
  - destruct (collect_rel rho _ _ _ _ _ _ _ HR HG HH Hc) as (H2' & Hc2 & HH').
    exists (OutOfMemory H2' (live_size H1')). split.
    + rewrite (live_size_rel _ _ _ HH'). apply alloc_overflow.
      * rewrite <- Hs; exact Hfull.
      * exact Hc2.
      * rewrite <- Hs, <- (live_size_rel _ _ _ HH'); exact Hover.
    + constructor; exact HH'.
Qed.

(** ** Observation *)

Lemma reify_rel {L1 L2} (rho : L1 -> L2 -> Prop) H1 H2 :
  heap_rel rho H1 H2 -> forall w1 v, reify H1 w1 v -> forall w2, word_rel rho w1 w2 -> reify H2 w2 v.
Proof.
  intros HH. fix IH 3. intros w1 v Hr w2 Hw. destruct Hr as [n | a s Hg | t | t a ws vs Hg Hws].
  - inversion Hw; subst. constructor.
  - inversion Hw; subst. constructor.
    destruct (hget_rel_some rho _ _ _ _ HH Hg) as (o2 & Hg2 & Ho). inversion Ho; subst. exact Hg2.
  - inversion Hw; subst. constructor.
  - inversion Hw; subst.
    destruct (hget_rel_some rho _ _ _ _ HH Hg) as (o2 & Hg2 & Ho). inversion Ho as [| ws1 ws2 Hf |]; subst.
    apply reify_con with ws2; [exact Hg2 |]. clear Hg Hg2 Ho Hw.
    revert ws2 Hf.
    induction Hws as [| x v' xs vs' Hxv Hrest IHl]; intros ws2 Hf; inversion Hf; subst; constructor.
    + eapply IH; eassumption.
    + apply IHl; assumption.
Qed.

Inductive outcome_rel {L1 L2} (rho : L1 -> L2 -> Prop) : outcome L1 -> outcome L2 -> Prop :=
| outr_halt w1 w2 : word_rel rho w1 w2 -> outcome_rel rho (Halt w1) (Halt w2)
| outr_trap e1 e2 : kind e1 = kind e2 -> outcome_rel rho (Trap e1) (Trap e2).

Lemma observe_rel {L1 L2} (rho : L1 -> L2 -> Prop) H1 H2 o1 o2 ob :
  heap_rel rho H1 H2 -> outcome_rel rho o1 o2 -> observe H1 o1 ob -> observe H2 o2 ob.
Proof.
  intros HH Ho Hob. destruct Hob as [w1 v Hr | w1 Hn | e1]; inversion Ho as [? w2 Hw | ? e2 Hk]; subst.
  - constructor. exact (reify_rel rho _ _ HH _ _ Hr _ Hw).
  - constructor. intros v Hr2. apply (Hn v).
    exact (reify_rel (flip_rel rho) _ _ (heap_rel_flip _ _ _ HH) _ _ Hr2 _ (word_rel_flip _ _ _ Hw)).
  - rewrite Hk. constructor.
Qed.

(** ** Registers *)

Lemma upd_rel {L1 L2} (rho : L1 -> L2 -> Prop) R1 R2 r w1 w2 :
  regs_rel rho R1 R2 -> word_rel rho w1 w2 -> regs_rel rho (upd R1 r w1) (upd R2 r w2).
Proof. intros HR Hw r'. unfold upd. destruct (Nat.eqb r' r); auto. Qed.

Lemma upd_from_rel {L1 L2} (rho : L1 -> L2 -> Prop) R1 R2 d ws1 ws2 :
  regs_rel rho R1 R2 -> Forall2 (word_rel rho) ws1 ws2 ->
  regs_rel rho (upd_from R1 d ws1) (upd_from R2 d ws2).
Proof.
  intros HR Hws; revert R1 R2 d HR; induction Hws; intros R1 R2 d HR; simpl; auto.
  apply IHHws. apply upd_rel; auto.
Qed.

Lemma map_regs_rel {L1 L2} (rho : L1 -> L2 -> Prop) R1 R2 rs :
  regs_rel rho R1 R2 -> Forall2 (word_rel rho) (map R1 rs) (map R2 rs).
Proof. intros HR; induction rs; simpl; auto. Qed.

Lemma clear_regs_rel {L1 L2} (rho : L1 -> L2 -> Prop) n1 n2 :
  rho n1 n2 -> regs_rel rho (clear_regs n1) (clear_regs n2).
Proof. intros Hn r. unfold clear_regs. destruct (Nat.eqb r NULL); constructor; auto. Qed.
