(** Proof that [AsmEmit.emit] preserves behaviours ([AsmEmit.emit_correct]).

    The asm program and its emitted code run in lockstep: each asm step is
    matched by one or more machine steps (several for an [encore], whose
    argument moves come first, and for a [match] that unpacks), between
    states whose registers, heaps and globals are equal up to [lab], the
    relation between asm code addresses and the instruction indices where
    their code was placed. *)

From Stdlib Require Import List PeanoNat Bool ZArith Lia.
Import ListNotations.
From EncorePasses Require Import Result Runtime RuntimeRel Correctness.
From EncorePasses Require Asm Prim Machine AsmSem AsmEmit.

Lemma bind_ok {A B} (m : result A) (k : A -> result B) x :
  bind m k = Ok x -> exists a, m = Ok a /\ k a = Ok x.
Proof. destruct m; simpl; [eauto | discriminate]. Qed.

(** ** Code layout *)

Section Layout.
  Variable C : list Machine.instr.

  (** [cs] sits in [C] at address [pc]. *)
  Definition code_at (pc : nat) (cs : list Machine.instr) : Prop :=
    forall i x, nth_error cs i = Some x -> nth_error C (pc + i) = Some x.

  Lemma code_at_cons pc x cs :
    code_at pc (x :: cs) -> nth_error C pc = Some x /\ code_at (S pc) cs.
  Proof.
    intros H. split.
    - rewrite <- (Nat.add_0_r pc). apply (H 0). reflexivity.
    - intros i y Hy. replace (S pc + i) with (pc + S i) by lia. apply (H (S i)). exact Hy.
  Qed.

  Lemma code_at_app pc a b :
    code_at pc (a ++ b) -> code_at pc a /\ code_at (pc + length a) b.
  Proof.
    intros H. split.
    - intros i x Hx. apply H. rewrite nth_error_app1; [exact Hx |].
      apply nth_error_Some. congruence.
    - intros i x Hx. rewrite <- Nat.add_assoc. apply H.
      rewrite nth_error_app2 by lia. replace (length a + i - length a) with i by lia. exact Hx.
  Qed.
End Layout.

(** ** The arity table *)

Lemma arity_of_In ps t n :
  AsmEmit.arities_consistent ps = true -> In (t, n) ps -> AsmEmit.arity_of ps t = n.
Proof.
  unfold AsmEmit.arities_consistent, AsmEmit.arity_of. intros Hc Hin.
  destruct (find (fun p => Nat.eqb (fst p) t) ps) as [[t' n'] |] eqn:E.
  - apply find_some in E as [Hin' Ht]. simpl in Ht. apply Nat.eqb_eq in Ht; subst t'.
    rewrite forallb_forall in Hc. specialize (Hc _ Hin'). rewrite forallb_forall in Hc.
    specialize (Hc _ Hin). simpl in Hc. rewrite Nat.eqb_refl in Hc. simpl in Hc.
    apply Nat.eqb_eq in Hc. exact Hc.
  - exfalso. apply (find_none _ _ E) in Hin. simpl in Hin. rewrite Nat.eqb_refl in Hin. discriminate.
Qed.

(** Words without code addresses are related only to themselves. *)
Lemma wr_clos_inv {L1 L2} (rho : L1 -> L2 -> Prop) a w : word_rel rho (WClos a) w -> w = WClos a.
Proof. inversion 1; auto. Qed.
Lemma wr_con_inv {L1 L2} (rho : L1 -> L2 -> Prop) t a w : word_rel rho (WCon t a) w -> w = WCon t a.
Proof. inversion 1; auto. Qed.
Lemma wr_int_inv {L1 L2} (rho : L1 -> L2 -> Prop) n w : word_rel rho (WInt n) w -> w = WInt n.
Proof. inversion 1; auto. Qed.
Lemma wr_bytes_inv {L1 L2} (rho : L1 -> L2 -> Prop) a w : word_rel rho (WBytes a) w -> w = WBytes a.
Proof. inversion 1; auto. Qed.
Lemma wr_func_inv {L1 L2} (rho : L1 -> L2 -> Prop) l w :
  word_rel rho (WFunc l) w -> exists l2, w = WFunc l2 /\ rho l l2.
Proof. inversion 1; eauto. Qed.

(** ** The simulation *)

Section Sim.
  (** The emitted program: its code, the context [emit] used, and its
      arity table. *)
  Variable C : list Machine.instr.
  Variable ng : nat.
  Variable sa : nat -> option nat.
  Variable pairs : list (nat * nat).
  Hypothesis Hcons : AsmEmit.arities_consistent pairs = true.
  Hypothesis Hstubs : forall j a, sa j = Some a -> code_at C a (AsmEmit.stub j).
  Hypothesis Hret : nth_error C 0 = Some (Machine.Fin A1).
  Hypothesis Hlen : length C <= Machine.null_label.
  Let ar := AsmEmit.arity_of pairs.

  (** The run: memory size, the asm program, the entry points, the host. *)
  Variable mem : nat.
  Variable prog : Asm.module.
  Variable entries : list Machine.label.
  Hypothesis Hent : length entries = length prog.
  Variable h : host_family.
  Hypothesis Hh : host_parametric h.

  (** [lab l pc]: the code for asm address [l] is at [pc]. *)
  Inductive lab : AsmSem.alabel -> nat -> Prop :=
  | lab_body e pc cs :
      AsmEmit.emit_expr ng sa pc e = Ok cs -> code_at C pc cs ->
      incl (AsmEmit.arities_expr e) pairs -> lab (AsmSem.Body e) pc
  | lab_stub j a : sa j = Some a -> lab (AsmSem.Stub j) a
  | lab_return : lab AsmSem.Return 0
  | lab_null : lab AsmSem.NullCode Machine.null_label.

  Record match_states (s1 : AsmSem.state) (s2 : Machine.state) : Prop := {
    ms_ctl : lab (AsmSem.ctl s1) (Machine.pc s2);
    ms_regs : regs_rel lab (AsmSem.R s1) (Machine.R s2);
    ms_heap : heap_rel lab (AsmSem.H s1) (Machine.H s2);
    ms_hp : AsmSem.hp s1 = Machine.hp s2;
    ms_glob : Forall2 (word_rel lab) (AsmSem.G s1) (Machine.G s2)
  }.

  Inductive match_result : AsmSem.step_result -> Machine.step_result -> Prop :=
  | mr_run s1 s2 : match_states s1 s2 -> match_result (AsmSem.Run s1) (Machine.Run s2)
  | mr_stop o1 o2 H1 H2 hp :
      outcome_rel lab o1 o2 -> heap_rel lab H1 H2 ->
      match_result (AsmSem.Stop o1 H1 hp) (Machine.Stop o2 H2 hp).

  Definition mstep := Machine.step C ar mem entries (h Machine.label).
  Definition astep := AsmSem.step mem prog (h AsmSem.alabel).

  (** One or more machine steps. *)
  Inductive msteps : Machine.state -> Machine.step_result -> Prop :=
  | msteps_one s r : mstep s r -> msteps s r
  | msteps_more s s' r : mstep s (Machine.Run s') -> msteps s' r -> msteps s r.

  Lemma cap_eq : AsmSem.cap mem prog = Machine.cap mem entries.
  Proof. unfold AsmSem.cap, Machine.cap. rewrite Hent. reflexivity. Qed.

  Lemma lab_body_inv e pc :
    lab (AsmSem.Body e) pc ->
    exists cs, AsmEmit.emit_expr ng sa pc e = Ok cs /\ code_at C pc cs /\
               incl (AsmEmit.arities_expr e) pairs.
  Proof. inversion 1; subst; eauto. Qed.

  (** [emit_expr] on a [let] of anything but a continuation. *)
  Lemma emit_let_val pc d v rest :
    (forall caps body, v <> Asm.ContLam caps body) ->
    AsmEmit.emit_expr ng sa pc (Asm.Let d v rest) =
    (let* i := AsmEmit.val_instr ng sa d v in
     let* cr := AsmEmit.emit_expr ng sa (S pc) rest in Ok (i :: cr)).
  Proof. intros Hv. destruct v; try reflexivity. exfalso. eapply Hv. reflexivity. Qed.

  Lemma let_val_inv pc d v rest cs :
    (forall caps body, v <> Asm.ContLam caps body) ->
    AsmEmit.emit_expr ng sa pc (Asm.Let d v rest) = Ok cs ->
    exists i cs', AsmEmit.val_instr ng sa d v = Ok i /\
                  AsmEmit.emit_expr ng sa (S pc) rest = Ok cs' /\ cs = i :: cs'.
  Proof.
    intros Hv He. rewrite emit_let_val in He by exact Hv.
    apply bind_ok in He as (i & Hi & He). apply bind_ok in He as (cs' & Hcs & He).
    injection He as <-. eauto.
  Qed.

  (** A [let] that only writes a register, on both sides. *)
  Lemma let_next s1 s2 d rest cs' w1 w2 :
    match_states s1 s2 ->
    AsmEmit.emit_expr ng sa (S (Machine.pc s2)) rest = Ok cs' -> code_at C (S (Machine.pc s2)) cs' ->
    incl (AsmEmit.arities_expr rest) pairs -> word_rel lab w1 w2 ->
    match_result (AsmSem.goto s1 rest (upd (AsmSem.R s1) d w1))
                 (Machine.next s2 (upd (Machine.R s2) d w2)).
  Proof.
    intros Hm He Hc Hi Hw. constructor. destruct Hm. constructor; simpl; auto.
    - econstructor; eauto.
    - apply upd_rel; auto.
  Qed.

  Lemma incl_let d v e :
    incl (AsmEmit.arities_expr (Asm.Let d v e)) pairs -> incl (AsmEmit.arities_expr e) pairs.
  Proof.
    intros Hi x Hx. apply Hi.
    change (In x (AsmEmit.arities_val v ++ AsmEmit.arities_expr e)). apply in_or_app. auto.
  Qed.

  Ltac body_code Hl :=
    match goal with Hc : AsmSem.ctl _ = AsmSem.Body _ |- _ =>
      rewrite Hc in Hl; apply lab_body_inv in Hl as (cs & He & Hcode & Hinc) end.

  Ltac let_start Hl :=
    body_code Hl;
    match goal with He : AsmEmit.emit_expr _ _ _ (Asm.Let _ _ _) = Ok _ |- _ =>
      apply let_val_inv in He as (ins & cs' & Hins & He & ->); [| intros ? ? ?; discriminate] end;
    match goal with Hcode : code_at _ _ (_ :: _) |- _ =>
      apply code_at_cons in Hcode as [Hf Hcode] end.

  (* the goal [match_result (goto ..) (next ..)] after a register-only let *)
  Ltac let_finish :=
    match goal with
    | Hm : match_states _ _, He : AsmEmit.emit_expr _ _ _ _ = Ok _, Hcode : code_at _ _ _,
      Hinc : incl _ _ |- _ =>
        apply (let_next _ _ _ _ _ _ _ Hm He Hcode (incl_let _ _ _ Hinc))
    end.

  (** Allocation, on both sides. *)
  Lemma alloc_step s1 s2 o1 o2 d mk1 mk2 rest cs' r1 :
    match_states s1 s2 ->
    AsmEmit.emit_expr ng sa (S (Machine.pc s2)) rest = Ok cs' -> code_at C (S (Machine.pc s2)) cs' ->
    incl (AsmEmit.arities_expr rest) pairs ->
    obj_rel lab o1 o2 -> (forall a, word_rel lab (mk1 a) (mk2 a)) ->
    AsmSem.allocate mem prog s1 o1 d mk1 rest r1 ->
    exists r2, Machine.allocate mem entries s2 o2 d mk2 r2 /\ match_result r1 r2.
  Proof.
    intros Hm He Hc Hi Ho Hmk Ha. destruct Hm as [Hl HR HH Hhp HG].
    destruct Ha as [H1' hp' a Hal | H1' hp' Hal]; rewrite cap_eq, Hhp in Hal;
      destruct (alloc_rel lab _ _ _ _ _ _ _ _ _ _ _ HR HG HH Ho Hal) as (res2 & Hal2 & Hres);
      inversion Hres as [? H2' ? ? HH' | ? H2' ? HH']; subst.
    - eexists. split; [apply Machine.allocate_ok; exact Hal2 |].
      constructor. constructor; simpl; auto.
      + econstructor; eauto.
      + apply upd_rel; auto.
    - eexists. split; [apply Machine.allocate_oom; exact Hal2 |].
      constructor; auto. constructor. reflexivity.
  Qed.

  (** [emit_expr] on a [cont-lam] or a [letrec]. *)
  Lemma closure_inv pc d caps body rest cs :
    AsmEmit.emit_expr ng sa pc (Asm.Let d (Asm.ContLam caps body) rest) = Ok cs \/
    AsmEmit.emit_expr ng sa pc (Asm.Letrec d caps body rest) = Ok cs ->
    exists i cr cb,
      AsmEmit.closure_instr d caps (S pc + length cr) = Ok i /\
      AsmEmit.emit_expr ng sa (S pc) rest = Ok cr /\
      AsmEmit.emit_expr ng sa (S pc + length cr) body = Ok cb /\ cs = i :: cr ++ cb.
  Proof.
    intros [He | He]; cbn -[AsmEmit.emit_expr AsmEmit.closure_instr] in He;
      apply bind_ok in He as (cr & Hr & He); apply bind_ok in He as (cb & Hb & He);
      apply bind_ok in He as (i & Hi & He); injection He as <-; eauto 7.
  Qed.

  (** Creating a function or closure, on both sides. *)
  Lemma closure_sim s1 s2 d caps body rest i cr cb r1 :
    match_states s1 s2 ->
    AsmEmit.closure_instr d caps (S (Machine.pc s2) + length cr) = Ok i ->
    nth_error C (Machine.pc s2) = Some i ->
    AsmEmit.emit_expr ng sa (S (Machine.pc s2)) rest = Ok cr -> code_at C (S (Machine.pc s2)) cr ->
    AsmEmit.emit_expr ng sa (S (Machine.pc s2) + length cr) body = Ok cb ->
    code_at C (S (Machine.pc s2) + length cr) cb ->
    incl (AsmEmit.arities_expr rest) pairs -> incl (AsmEmit.arities_expr body) pairs ->
    AsmSem.make_closure mem prog s1 d caps body rest r1 ->
    exists r2, mstep s2 r2 /\ match_result r1 r2.
  Proof.
    intros Hm Hi Hf Hr Hcr Hb Hcb Hir Hib Hmk.
    assert (Hlb : lab (AsmSem.Body body) (S (Machine.pc s2) + length cr)) by (econstructor; eauto).
    destruct Hmk as [Hnil | res Hne Ha].
    - subst caps. unfold AsmEmit.closure_instr in Hi. injection Hi as <-.
      eexists. split; [apply Machine.s_function; exact Hf |].
      apply (let_next _ _ _ _ _ _ _ Hm Hr Hcr Hir). constructor. exact Hlb.
    - destruct caps as [| c caps]; [contradiction |].
      unfold AsmEmit.closure_instr in Hi. cbv beta iota in Hi.
      destruct (Nat.leb (length (c :: caps)) AsmEmit.max_captures); [injection Hi as <- | discriminate].
      assert (Ho : obj_rel lab (OClosure (AsmSem.Body body) (map (AsmSem.R s1) (c :: caps)))
                               (OClosure (S (Machine.pc s2) + length cr) (map (Machine.R s2) (c :: caps)))).
      { constructor; [exact Hlb | apply map_regs_rel, (ms_regs _ _ Hm)]. }
      destruct (alloc_step _ _ _ _ _ _ WClos _ _ _ Hm Hr Hcr Hir Ho (fun a => wr_clos lab a) Ha)
        as (r2 & Ha2 & Hres).
      exists r2. split; [eapply Machine.s_closure; [exact Hf | exact Ha2] | exact Hres].
  Qed.

End Sim.
