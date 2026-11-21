(** Proof that [AsmEmit.emit] preserves behaviours ([AsmEmit.emit_correct]).

    The asm program and its emitted code run in lockstep: each asm step is
    matched by one or more machine steps (several for an [encore], whose
    argument moves come first, and for a [match] that unpacks), between
    states whose registers, heaps and globals are equal up to [lab], the
    relation between asm code addresses and the instruction indices where
    their code was placed. *)

From Stdlib Require Import List PeanoNat Bool ZArith Lia.
From Stdlib Require String.
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
  Inductive lab : AsmSem.alabel -> Machine.label -> Prop :=
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

  Lemma as_int_rel w1 w2 n :
    word_rel lab w1 w2 -> AsmSem.as_int w1 = Some n -> Machine.as_int w2 = Some n.
  Proof. destruct 1; simpl; congruence. Qed.

  Lemma as_int_rel_none w1 w2 :
    word_rel lab w1 w2 -> AsmSem.as_int w1 = None -> Machine.as_int w2 = None.
  Proof. destruct 1; simpl; congruence. Qed.

  Lemma bytes_of_rel s1 s2 w1 w2 b :
    match_states s1 s2 -> word_rel lab w1 w2 ->
    AsmSem.bytes_of s1 w1 = Some b -> Machine.bytes_of s2 w2 = Some b.
  Proof.
    intros Hm Hw Hb. destruct Hw; simpl in *; try discriminate.
    destruct (hget (AsmSem.H s1) a) as [o1 |] eqn:E; [| discriminate].
    destruct (hget_rel_some _ _ _ _ _ (ms_heap _ _ Hm) E) as (o2 & -> & Ho).
    inversion Ho; subst; congruence.
  Qed.

  Lemma prim_word_rel r : word_rel lab (prim_word r) (prim_word r).
  Proof. destruct r; constructor. Qed.

  Lemma bool_word_rel b : word_rel lab (bool_word b) (bool_word b).
  Proof. constructor. Qed.

  Lemma trap_match s1 s2 e1 e2 :
    match_states s1 s2 -> kind e1 = kind e2 ->
    match_result (AsmSem.trap s1 e1) (Machine.trap s2 e2).
  Proof.
    intros Hm Hk. unfold AsmSem.trap, Machine.trap. rewrite (ms_hp _ _ Hm).
    constructor; [constructor; exact Hk | exact (ms_heap _ _ Hm)].
  Qed.

  Lemma prim_binop_instr d o b x y :
    AsmSem.int_binop o = Some b ->
    AsmEmit.prim_instr d (Prim.PInt o) [x; y] = Ok (Machine.IntBin b d x y).
  Proof. destruct o; simpl; congruence. Qed.

  (** [R1 r] is an integer exactly when [R2 r] is, with the same value. *)
  Ltac int_reg Hm r H :=
    let Hr := fresh "Hr" in
    pose proof (ms_regs _ _ Hm r) as Hr; rewrite H in Hr; apply wr_int_inv in Hr.

  (** Zero or more machine steps. *)
  Inductive mstar : Machine.state -> Machine.state -> Prop :=
  | mstar_refl s : mstar s s
  | mstar_step s s' s'' : mstep s (Machine.Run s') -> mstar s' s'' -> mstar s s''.

  Lemma mstar_msteps s s' r : mstar s s' -> mstep s' r -> msteps s r.
  Proof.
    induction 1 as [| s s' s'' Hs _ IH]; intros Hr; [apply msteps_one; exact Hr |].
    eapply msteps_more; [exact Hs | apply IH; exact Hr].
  Qed.

  Lemma upd_from_ext {L} (R R' : regs L) d ws :
    (forall r, R r = R' r) -> forall r, upd_from R d ws r = upd_from R' d ws r.
  Proof.
    revert R R' d. induction ws as [| w ws IH]; intros R R' d HR r; simpl; auto.
    apply IH. intros r'. unfold upd. destruct (Nat.eqb r' d); auto.
  Qed.

  Lemma map_upd_notin {L} (R : regs L) t w rest :
    existsb (Nat.eqb t) rest = false -> map (upd R t w) rest = map R rest.
  Proof.
    induction rest as [| x rest IH]; simpl; auto. intros Hx.
    apply orb_false_iff in Hx as [Htx Hr]. f_equal; [| apply IH; exact Hr].
    unfold upd. rewrite Nat.eqb_sym, Htx. reflexivity.
  Qed.

  (** The argument moves implement the parallel assignment. *)
  Lemma moves_exec args : forall s i f k,
    code_at C (Machine.pc s) (AsmEmit.moves i args) ->
    AsmEmit.no_clobber f k i args = true ->
    exists s', mstar s s' /\ Machine.pc s' = Machine.pc s + length (AsmEmit.moves i args) /\
      Machine.H s' = Machine.H s /\ Machine.hp s' = Machine.hp s /\ Machine.G s' = Machine.G s /\
      (forall r, Machine.R s' r = upd_from (Machine.R s) (A1 + i) (map (Machine.R s) args) r) /\
      Machine.R s' f = Machine.R s f /\ Machine.R s' k = Machine.R s k.
  Proof.
    induction args as [| x rest IH]; intros s i f k Hc Hn.
    - exists s. simpl. repeat split; auto using mstar_refl.
    - cbn [AsmEmit.moves AsmEmit.no_clobber] in Hc, Hn |- *. apply andb_true_iff in Hn as [Hx Hn].
      destruct (Nat.eqb_spec x (A1 + i)) as [Heq | Hne].
      + subst x. cbn [app] in Hc |- *.
        destruct (IH s (S i) f k Hc Hn) as (s' & Hs & Hpc & HH & Hhp & HG & HR & Hf & Hk).
        exists s'. repeat split; auto.
        intros r. rewrite HR. replace (A1 + S i) with (S (A1 + i)) by (unfold A1; lia).
        cbn [map upd_from]. apply upd_from_ext. intros r'. unfold upd.
        destruct (Nat.eqb_spec r' (A1 + i)); subst; reflexivity.
      + cbn [orb app] in Hc, Hx |- *.
        apply andb_true_iff in Hx as [Hx Hk]. apply andb_true_iff in Hx as [Hx Hf].
        apply negb_true_iff in Hx, Hf, Hk.
        apply code_at_cons in Hc as [Hm Hc].
        destruct (IH (Machine.St (S (Machine.pc s)) (upd (Machine.R s) (A1 + i) (Machine.R s x))
                                 (Machine.H s) (Machine.hp s) (Machine.G s)) (S i) f k Hc Hn)
          as (s' & Hs & Hpc & HH & Hhp & HG & HR & Hf' & Hk').
        exists s'. split; [eapply mstar_step; [apply Machine.s_mov; exact Hm | exact Hs] |].
        cbn [Machine.pc Machine.R Machine.H Machine.hp Machine.G] in Hpc, HH, Hhp, HG, HR, Hf', Hk'.
        repeat split.
        * rewrite Hpc. cbn [length]. lia.
        * exact HH.
        * exact Hhp.
        * exact HG.
        * intros r. rewrite HR. rewrite (map_upd_notin _ _ _ _ Hx).
          replace (A1 + S i) with (S (A1 + i)) by (unfold A1; lia). reflexivity.
        * rewrite Hf'. unfold upd. rewrite Hf. reflexivity.
        * rewrite Hk'. unfold upd. rewrite Hk. reflexivity.
  Qed.

  Lemma encore_inv pc f args k cs :
    AsmEmit.emit_expr ng sa pc (Asm.Encore f args k) = Ok cs ->
    AsmEmit.no_clobber f k 0 args = true /\ cs = AsmEmit.moves 0 args ++ [Machine.Encore f k].
  Proof.
    cbn [AsmEmit.emit_expr]. destruct (AsmEmit.no_clobber f k 0 args); [| discriminate].
    intros [= <-]. auto.
  Qed.

  Lemma emit_nonempty pc e cs : AsmEmit.emit_expr ng sa pc e = Ok cs -> cs <> [].
  Proof.
    destruct e as [d v rest | d caps body rest | f args k | r b cases | r]; intros He.
    - destruct v; cbn [AsmEmit.emit_expr] in He;
        repeat (apply bind_ok in He as (? & ? & He)); injection He as <-; discriminate.
    - cbn [AsmEmit.emit_expr] in He.
      repeat (apply bind_ok in He as (? & ? & He)). injection He as <-. discriminate.
    - apply encore_inv in He as [_ ->]. destruct (AsmEmit.moves 0 args); discriminate.
    - cbn [AsmEmit.emit_expr] in He. apply bind_ok in He as (? & ? & He). injection He as <-. discriminate.
    - cbn [AsmEmit.emit_expr] in He. injection He as <-. discriminate.
  Qed.

  Lemma code_of_rel H1 H2 w1 w2 l1 :
    heap_rel lab H1 H2 -> word_rel lab w1 w2 -> AsmSem.code_of H1 w1 = Some l1 ->
    exists l2, Machine.code_of H2 w2 = Some l2 /\ lab l1 l2.
  Proof.
    intros HH Hw Hc. destruct Hw as [| l l2 Hl | a | |]; simpl in Hc; try discriminate.
    - injection Hc as <-. exists l2. split; [reflexivity | exact Hl].
    - destruct (hget H1 a) as [o1 |] eqn:E; [| discriminate].
      destruct (hget_rel_some _ _ _ _ _ HH E) as (o2 & Hg & Ho). simpl. rewrite Hg.
      destruct Ho as [l1' l2' e1 e2 Hl Henv | |]; try discriminate.
      injection Hc as <-. eauto.
  Qed.

  Lemma lab_code l1 l2 : lab l1 l2 -> l1 <> AsmSem.NullCode -> exists i, nth_error C l2 = Some i.
  Proof.
    destruct 1 as [e pc cs He Hc _ | j a Hsa | |]; intros Hn.
    - destruct cs as [| i cs]; [exfalso; exact (emit_nonempty _ _ _ He eq_refl) |].
      exists i. exact (proj1 (code_at_cons _ _ _ _ Hc)).
    - exists (Machine.Extern X01 A1 j). exact (proj1 (code_at_cons _ _ _ _ (Hstubs _ _ Hsa))).
    - eauto.
    - contradiction.
  Qed.

  Lemma null_outside : nth_error C Machine.null_label = None.
  Proof. apply nth_error_None. exact Hlen. Qed.

  Lemma match_inv pc r b cases cs :
    AsmEmit.emit_expr ng sa pc (Asm.Match r b cases) = Ok cs ->
    exists blocks,
      AsmEmit.emit_cases (AsmEmit.emit_expr ng sa) r b (S pc) 0 cases = Ok blocks /\
      cs = AsmEmit.header_instr r b (map fst blocks) :: flat_map snd blocks.
  Proof.
    cbn [AsmEmit.emit_expr]. intros He. apply bind_ok in He as (blocks & Hb & He).
    injection He as <-. eauto.
  Qed.

  (** Case [j] of a [match]: its code starts at the [j]-th label. *)
  Lemma emit_cases_nth r b cs : forall at_ i blocks j n ub body,
    AsmEmit.emit_cases (AsmEmit.emit_expr ng sa) r b at_ i cs = Ok blocks ->
    code_at C at_ (flat_map snd blocks) ->
    nth_error cs j = Some (n, ub, body) ->
    exists l code, nth_error (map fst blocks) j = Some l /\ code_at C l code /\
      (n = 0 /\ AsmEmit.emit_expr ng sa l body = Ok code \/
       n > 0 /\ exists cb, AsmEmit.emit_expr ng sa (S l) body = Ok cb /\
                           code = Machine.Unpack ub (b + (i + j)) r :: cb).
  Proof.
    induction cs as [| [[n0 ub0] body0] cs IH]; intros at_ i blocks j n ub body He Hc Hj;
      [destruct j; discriminate |].
    cbn [AsmEmit.emit_cases] in He. apply bind_ok in He as (code & Hcode & He).
    apply bind_ok in He as (rest & Hrest & He). injection He as <-.
    cbn [flat_map map snd fst] in Hc |- *. apply code_at_app in Hc as [Hc0 Hcr].
    destruct j as [| j].
    - injection Hj as -> -> ->. exists at_, code. split; [reflexivity |]. split; [exact Hc0 |].
      destruct (Nat.eqb_spec n 0) as [-> | Hn].
      + left. auto.
      + right. split; [lia |]. destruct (Nat.leb (ub + n) n_regs); [| discriminate].
        apply bind_ok in Hcode as (cb & Hcb & Hcode). injection Hcode as <-.
        exists cb. split; [exact Hcb |]. rewrite Nat.add_0_r. reflexivity.
    - cbn [nth_error] in Hj |- *.
      destruct (IH _ _ _ _ _ _ _ Hrest Hcr Hj) as (l & code' & Hl & Hcl & Hcase).
      exists l, code'. split; [exact Hl |]. split; [exact Hcl |].
      replace (i + S j) with (S i + j) by lia. exact Hcase.
  Qed.

  Lemma arities_cases_nth (f : Asm.expr -> list (nat * nat)) b cs : forall i j n ub body,
    nth_error cs j = Some (n, ub, body) ->
    incl (AsmEmit.arities_cases f b i cs) pairs ->
    incl (f body) pairs /\ (n > 0 -> In (b + (i + j), n) pairs).
  Proof.
    induction cs as [| [[n0 ub0] body0] cs IH]; intros i j n ub body Hj Hinc; [destruct j; discriminate |].
    cbn [AsmEmit.arities_cases] in Hinc. destruct j as [| j].
    - injection Hj as -> -> ->. split.
      + intros x Hx. apply Hinc. apply in_or_app. right. apply in_or_app. left. exact Hx.
      + intros Hn. apply Hinc. apply in_or_app. left.
        destruct (Nat.eqb_spec n 0); [lia |]. rewrite Nat.add_0_r. left. reflexivity.
    - cbn [nth_error] in Hj.
      assert (Hinc' : incl (AsmEmit.arities_cases f b (S i) cs) pairs).
      { intros x Hx. apply Hinc. apply in_or_app. right. apply in_or_app. right. exact Hx. }
      destruct (IH _ _ _ _ _ Hj Hinc') as [Hb Hn]. split; [exact Hb |].
      intros Hpos. replace (i + S j) with (S i + j) by lia. apply Hn. exact Hpos.
  Qed.

  Lemma match_header s2 r b ls t oa l :
    nth_error C (Machine.pc s2) = Some (AsmEmit.header_instr r b ls) ->
    Machine.R s2 r = WCon t oa -> b <= t -> t < 256 -> nth_error ls (t - b) = Some l ->
    mstep s2 (Machine.Run (Machine.St l (Machine.R s2) (Machine.H s2) (Machine.hp s2) (Machine.G s2))).
  Proof.
    intros Hf Hr Hb Ht Hl.
    assert (Hmod : (t + 256 - b) mod 256 = t - b).
    { replace (t + 256 - b) with (t - b + 1 * 256) by lia. rewrite Nat.Div0.mod_add.
      apply Nat.mod_small. lia. }
    destruct ls as [| l0 [| l1 [| l2 ls]]].
    - destruct (t - b); discriminate.
    - eapply Machine.s_match; [exact Hf | exact Hr | rewrite Hmod; exact Hl].
    - replace l with (if Nat.eqb t b then l0 else l1).
      + eapply Machine.s_branch; [exact Hf | exact Hr].
      + destruct (Nat.eqb_spec t b) as [-> | Hne].
        * rewrite Nat.sub_diag in Hl. injection Hl as <-. reflexivity.
        * destruct (t - b) as [| [| k]] eqn:E; [lia | injection Hl as <-; reflexivity |].
          destruct k; discriminate.
    - eapply Machine.s_match; [exact Hf | exact Hr | rewrite Hmod; exact Hl].
  Qed.

  Lemma Forall2_firstn {A B} (P : A -> B -> Prop) n l1 l2 :
    Forall2 P l1 l2 -> Forall2 P (firstn n l1) (firstn n l2).
  Proof.
    intros H. revert n. induction H as [| x y l1 l2 Hxy _ IH]; intros [| n]; simpl; auto.
  Qed.

  Lemma step_sim s1 r1 s2 :
    astep s1 r1 -> match_states s1 s2 -> exists r2, msteps s2 r2 /\ match_result r1 r2.
  Proof.
    intros Hstep Hm. pose proof (ms_ctl _ _ Hm) as Hl. unfold astep in Hstep.
    destruct Hstep.
    - (* reg *) body_code Hl.
      apply let_val_inv in He as (ins & cs' & Hins & He & ->); [| intros ? ? ?; discriminate]. cbn in Hins. injection Hins as <-.
      apply code_at_cons in Hcode as [Hf Hcode].
      eexists. split.
      + apply msteps_one. apply Machine.s_mov. exact Hf.
      + let_finish. apply (ms_regs _ _ Hm).
    - (* capture *) let_start Hl. cbn in Hins. injection Hins as <-.
      pose proof (ms_regs _ _ Hm SELF) as Hself. rewrite H0 in Hself. apply wr_clos_inv in Hself.
      destruct (hget_rel_some _ _ _ _ _ (ms_heap _ _ Hm) H1) as (o2 & Hg2 & Ho).
      inversion Ho as [l1 l2 env1 env2 Hl12 Henv | |]; subst.
      destruct (Forall2_nth_error _ _ _ _ _ Henv H2) as (w2 & Hw2 & Hw).
      exists (Machine.next s2 (upd (Machine.R s2) d w2)). split.
      + apply msteps_one. eapply Machine.s_capture; [exact Hf | exact Hself | exact Hg2 | exact Hw2].
      + let_finish. exact Hw.
    - (* global *) let_start Hl. unfold AsmEmit.val_instr in Hins.
      destruct (Forall2_nth_error _ _ _ _ _ (ms_glob _ _ Hm) H0) as (w2 & Hg2 & Hw).
      destruct (Nat.ltb i ng); [| discriminate].
      destruct (Nat.ltb i 256); injection Hins as <-; eexists; split.
      + apply msteps_one. eapply Machine.s_global; [exact Hf | exact Hg2].
      + let_finish. exact Hw.
      + apply msteps_one. eapply Machine.s_global_w; [exact Hf | exact Hg2].
      + let_finish. exact Hw.
    - (* cont-lam *) body_code Hl.
      destruct (closure_inv _ _ _ _ _ _ (or_introl He)) as (ins & cr & cb & Hins & Hr & Hb & ->).
      apply code_at_cons in Hcode as [Hf Hcode]. apply code_at_app in Hcode as [Hcr Hcb].
      assert (Hib : incl (AsmEmit.arities_expr body) pairs).
      { intros x Hx. apply Hinc. change (In x (AsmEmit.arities_expr body ++ AsmEmit.arities_expr e)).
        apply in_or_app. auto. }
      destruct (closure_sim _ _ _ _ _ _ _ _ _ _ Hm Hins Hf Hr Hcr Hb Hcb (incl_let _ _ _ Hinc) Hib H0)
        as (r2 & Hs2 & Hres).
      exists r2. split; [apply msteps_one; exact Hs2 | exact Hres].
    - (* nullary constructor *) let_start Hl. unfold AsmEmit.val_instr in Hins. cbn in Hins.
      injection Hins as <-.
      eexists. split.
      + apply msteps_one. apply Machine.s_pack_nullary; [exact Hf |].
        apply arity_of_In; [exact Hcons |]. apply Hinc. left. reflexivity.
      + let_finish. constructor.
    - (* constructor *) let_start Hl. unfold AsmEmit.val_instr in Hins.
      destruct (Nat.leb (length fs) AsmEmit.max_ctor_arity); [injection Hins as <- | discriminate].
      assert (Har : ar t = length fs).
      { apply arity_of_In; [exact Hcons |]. apply Hinc. left. reflexivity. }
      assert (Ho : obj_rel lab (OFields (map (AsmSem.R s1) fs)) (OFields (map (Machine.R s2) fs))).
      { constructor. apply map_regs_rel, (ms_regs _ _ Hm). }
      destruct (alloc_step _ _ _ _ _ _ (fun a => WCon t (Some a)) _ _ _ Hm He Hcode (incl_let _ _ _ Hinc)
                  Ho (fun a => wr_con lab t (Some a)) H1) as (r2 & Ha2 & Hres).
      exists r2. split; [| exact Hres]. apply msteps_one.
      eapply Machine.s_pack; [exact Hf | | | exact Ha2].
      + rewrite Har. destruct fs; [contradiction | simpl; lia].
      + symmetry. exact Har.
    - (* field *) let_start Hl. cbn in Hins. injection Hins as <-.
      pose proof (ms_regs _ _ Hm r) as Hr. rewrite H0 in Hr. apply wr_con_inv in Hr.
      destruct (hget_rel_some _ _ _ _ _ (ms_heap _ _ Hm) H1) as (o2 & Hg2 & Ho).
      inversion Ho as [| ws1 ws2 Hws |]; subst.
      destruct (Forall2_nth_error _ _ _ _ _ Hws H2) as (w2 & Hw2 & Hw).
      eexists. split.
      + apply msteps_one. eapply Machine.s_field; [exact Hf | exact Hr | exact Hg2 | exact Hw2].
      + let_finish. exact Hw.
    - (* integer literal *) let_start Hl. unfold AsmEmit.val_instr in Hins.
      destruct (Z.eqb_spec n 0) as [-> | _]; [injection Hins as <-; eexists; split;
        [apply msteps_one; apply Machine.s_int0; exact Hf | let_finish; constructor] |].
      destruct (Z.eqb_spec n 1) as [-> | _]; [injection Hins as <-; eexists; split;
        [apply msteps_one; apply Machine.s_int1; exact Hf | let_finish; constructor] |].
      destruct (Z.eqb_spec n 2) as [-> | _]; [injection Hins as <-; eexists; split;
        [apply msteps_one; apply Machine.s_int2; exact Hf | let_finish; constructor] |].
      destruct (in_range n); [injection Hins as <- | discriminate].
      eexists. split; [apply msteps_one; apply Machine.s_int; exact Hf | let_finish; constructor].
    - (* byte-string literal *) let_start Hl. unfold AsmEmit.val_instr in Hins.
      destruct (Nat.leb (length (String.list_byte_of_string b)) AsmEmit.max_bytes_literal);
        [injection Hins as <- | discriminate].
      destruct (alloc_step _ _ (OBytes (String.list_byte_of_string b)) (OBytes (String.list_byte_of_string b)) _ _ WBytes
                  _ _ _ Hm He Hcode (incl_let _ _ _ Hinc) (or_bytes lab _) (fun a => wr_bytes lab a) H0)
        as (r2 & Ha2 & Hres).
      exists r2. split; [apply msteps_one; eapply Machine.s_bytes; [exact Hf | exact Ha2] | exact Hres].
    - (* extern *) let_start Hl. unfold AsmEmit.val_instr in Hins.
      destruct (sa j) as [a |] eqn:Esa; [| discriminate].
      destruct (Nat.ltb j AsmEmit.max_extern); [injection Hins as <- | discriminate].
      eexists. split; [apply msteps_one; apply Machine.s_function; exact Hf |].
      let_finish. constructor. constructor. exact Esa.
    - (* binop *) let_start Hl. unfold AsmEmit.val_instr in Hins.
      rewrite (prim_binop_instr _ _ _ _ _ H0) in Hins. injection Hins as <-.
      eexists. split.
      + apply msteps_one. eapply Machine.s_intbin; [exact Hf | | | exact H3];
          eapply as_int_rel; eauto; apply (ms_regs _ _ Hm).
      + let_finish. apply prim_word_rel.
    - (* binop overflow *) let_start Hl. unfold AsmEmit.val_instr in Hins.
      rewrite (prim_binop_instr _ _ _ _ _ H0) in Hins. injection Hins as <-.
      eexists. split.
      + apply msteps_one. eapply Machine.s_intbin_overflow; [exact Hf | | | exact H3];
          eapply as_int_rel; eauto; apply (ms_regs _ _ Hm).
      + apply trap_match; auto.
    - (* binop type error *) let_start Hl. unfold AsmEmit.val_instr in Hins.
      rewrite (prim_binop_instr _ _ _ _ _ H0) in Hins. injection Hins as <-.
      eexists. split.
      + apply msteps_one. eapply Machine.s_intbin_type; [exact Hf |].
        destruct H1 as [Hx | Hy]; [left | right]; eapply as_int_rel_none; eauto; apply (ms_regs _ _ Hm).
      + apply trap_match; auto.
    - (* int.byte *) let_start Hl. cbn in Hins. injection Hins as <-. int_reg Hm x H0.
      destruct (alloc_step _ _ (OBytes [c]) (OBytes [c]) _ _ WBytes _ _ _ Hm He Hcode (incl_let _ _ _ Hinc)
                  (or_bytes lab _) (fun a => wr_bytes lab a) H3) as (r2 & Ha2 & Hres).
      exists r2. split; [| exact Hres]. apply msteps_one.
      eapply Machine.s_int_byte; [exact Hf | exact Hr | exact H2 | exact H1 | exact Ha2].
    - (* int.byte out of range *) let_start Hl. cbn in Hins. injection Hins as <-. int_reg Hm x H0.
      eexists. split; [apply msteps_one; eapply Machine.s_int_byte_range; [exact Hf | exact Hr | exact H1] |].
      apply trap_match; auto.
    - (* int.byte type error *) let_start Hl. cbn in Hins. injection Hins as <-.
      eexists. split; [apply msteps_one; eapply Machine.s_int_byte_type; [exact Hf |] |].
      + eapply as_int_rel_none; eauto. apply (ms_regs _ _ Hm).
      + apply trap_match; auto.
    - (* bytes.len *) let_start Hl. cbn in Hins. injection Hins as <-.
      eexists. split; [apply msteps_one; eapply Machine.s_bytes_len; [exact Hf |] |].
      + eapply bytes_of_rel; eauto. apply (ms_regs _ _ Hm).
      + let_finish. constructor.
    - (* bytes.get *) let_start Hl. cbn in Hins. injection Hins as <-. int_reg Hm i H0.
      eexists. split; [apply msteps_one; eapply Machine.s_bytes_get; [exact Hf | exact Hr | | exact H2 | exact H3] |].
      + eapply bytes_of_rel; eauto. apply (ms_regs _ _ Hm).
      + let_finish. constructor.
    - (* bytes.get type error *) let_start Hl. cbn in Hins. injection Hins as <-.
      eexists. split; [apply msteps_one; eapply Machine.s_bytes_get_type; [exact Hf |] |].
      + eapply as_int_rel_none; eauto. apply (ms_regs _ _ Hm).
      + apply trap_match; auto.
    - (* bytes.concat *) let_start Hl. cbn in Hins. injection Hins as <-.
      destruct (alloc_step _ _ (OBytes (b1 ++ b2)) (OBytes (b1 ++ b2)) _ _ WBytes _ _ _ Hm He Hcode
                  (incl_let _ _ _ Hinc) (or_bytes lab _) (fun a => wr_bytes lab a) H2) as (r2 & Ha2 & Hres).
      exists r2. split; [| exact Hres]. apply msteps_one.
      eapply Machine.s_bytes_concat; [exact Hf | | | exact Ha2]; eapply bytes_of_rel; eauto; apply (ms_regs _ _ Hm).
    - (* bytes.slice *) let_start Hl. cbn in Hins. injection Hins as <-.
      int_reg Hm i H0. rename Hr into Hri. int_reg Hm n H1.
      destruct (alloc_step _ _ _ _ _ _ WBytes _ _ _ Hm He Hcode (incl_let _ _ _ Hinc)
                  (or_bytes lab _) (fun a => wr_bytes lab a) H6) as (r2 & Ha2 & Hres).
      exists r2. split; [| exact Hres]. apply msteps_one.
      eapply Machine.s_bytes_slice; [exact Hf | exact Hri | exact Hr | exact H2 | exact H3 | | exact H5 | exact Ha2].
      eapply bytes_of_rel; eauto. apply (ms_regs _ _ Hm).
    - (* bytes.slice out of memory *) let_start Hl. cbn in Hins. injection Hins as <-.
      int_reg Hm i H0. rename Hr into Hri. int_reg Hm n H1.
      destruct Hm as [Hl' HR HH Hhp HG].
      rewrite cap_eq, Hhp in H3.
      destruct (alloc_rel lab _ _ _ _ _ _ _ _ _ _ _ HR HG HH (or_bytes lab _) H3) as (res2 & Hal2 & Hres).
      inversion Hres as [| ? H2' ? HH']; subst.
      eexists. split.
      + apply msteps_one. eapply Machine.s_bytes_slice_oom; [exact Hf | exact Hri | exact Hr | exact H2 | exact Hal2].
      + constructor; [constructor; reflexivity | exact HH'].
    - (* bytes.slice type error *) let_start Hl. cbn in Hins. injection Hins as <-.
      eexists. split; [apply msteps_one; eapply Machine.s_bytes_slice_type; [exact Hf |] |].
      + destruct H0 as [Hx | Hy]; [left | right]; eapply as_int_rel_none; eauto; apply (ms_regs _ _ Hm).
      + apply trap_match; auto.
    - (* bytes.eq *) let_start Hl. cbn in Hins. injection Hins as <-.
      eexists. split.
      + apply msteps_one. eapply Machine.s_bytes_eq; [exact Hf | |].
        * exact (bytes_of_rel _ _ _ _ _ Hm (ms_regs _ _ Hm x) H0).
        * exact (bytes_of_rel _ _ _ _ _ Hm (ms_regs _ _ Hm y) H1).
      + let_finish. apply bool_word_rel.
    - (* letrec *) body_code Hl.
      destruct (closure_inv _ _ _ _ _ _ (or_intror He)) as (ins & cr & cb & Hins & Hr & Hb & ->).
      apply code_at_cons in Hcode as [Hf Hcode]. apply code_at_app in Hcode as [Hcr Hcb].
      assert (Hib : incl (AsmEmit.arities_expr body) pairs).
      { intros x Hx. apply Hinc. change (In x (AsmEmit.arities_expr body ++ AsmEmit.arities_expr e)).
        apply in_or_app. auto. }
      assert (Hie : incl (AsmEmit.arities_expr e) pairs).
      { intros x Hx. apply Hinc. change (In x (AsmEmit.arities_expr body ++ AsmEmit.arities_expr e)).
        apply in_or_app. auto. }
      destruct (closure_sim _ _ _ _ _ _ _ _ _ _ Hm Hins Hf Hr Hcr Hb Hcb Hie Hib H0) as (r2 & Hs2 & Hres).
      exists r2. split; [apply msteps_one; exact Hs2 | exact Hres].
    - (* encore *) body_code Hl. apply encore_inv in He as [Hn ->].
      apply code_at_app in Hcode as [Hmv Hen]. apply code_at_cons in Hen as [Hfe _].
      destruct (moves_exec args s2 0 f k Hmv Hn) as (s2' & Hst & Hpc & HH & Hhp & HG & HR & Hf & Hk).
      destruct (code_of_rel _ _ _ _ _ (ms_heap _ _ Hm) (ms_regs _ _ Hm f) H0) as (l2 & Hc2 & Hl2).
      destruct (lab_code _ _ Hl2 H1) as (i2 & Hi2).
      eexists. split.
      + eapply mstar_msteps; [exact Hst |]. eapply Machine.s_encore.
        * unfold Machine.fetch. rewrite Hpc. exact Hfe.
        * rewrite HH, Hf. exact Hc2.
        * exact Hi2.
      + constructor. destruct Hm as [_ HR12 HH12 Hhp12 HG12]. constructor; simpl.
        * exact Hl2.
        * unfold AsmSem.call_regs. apply upd_rel; [apply upd_rel |].
          -- intros r. rewrite HR. rewrite Nat.add_0_r.
             apply upd_from_rel; [exact HR12 | apply map_regs_rel; exact HR12].
          -- rewrite Hf. apply HR12.
          -- rewrite Hk. apply HR12.
        * rewrite HH. exact HH12.
        * rewrite Hhp. exact Hhp12.
        * rewrite HG. exact HG12.
    - (* encore to NULL *) body_code Hl. apply encore_inv in He as [Hn ->].
      apply code_at_app in Hcode as [Hmv Hen]. apply code_at_cons in Hen as [Hfe _].
      destruct (moves_exec args s2 0 f k Hmv Hn) as (s2' & Hst & Hpc & HH & Hhp & HG & HR & Hf & Hk).
      destruct (code_of_rel _ _ _ _ _ (ms_heap _ _ Hm) (ms_regs _ _ Hm f) H0) as (l2 & Hc2 & Hl2).
      inversion Hl2; subst.
      eexists. split.
      + eapply mstar_msteps; [exact Hst |]. eapply Machine.s_encore_outside.
        * unfold Machine.fetch. rewrite Hpc. exact Hfe.
        * rewrite HH, Hf. exact Hc2.
        * exact null_outside.
      + unfold AsmSem.trap, Machine.trap. rewrite HH, Hhp, <- (ms_hp _ _ Hm).
        constructor; [constructor; reflexivity | exact (ms_heap _ _ Hm)].
    - (* match, no fields *) body_code Hl. apply match_inv in He as (blocks & Hb & ->).
      apply code_at_cons in Hcode as [Hf Hcode].
      destruct (emit_cases_nth _ _ _ _ _ _ _ _ _ _ Hb Hcode H3) as (l & code & Hlj & Hcl & [[_ Hel] | [Hn _]]);
        [| lia].
      destruct (arities_cases_nth AsmEmit.arities_expr b cases 0 _ _ _ _ H3 Hinc) as [Hib _].
      pose proof (ms_regs _ _ Hm r) as Hr. rewrite H0 in Hr. apply wr_con_inv in Hr.
      eexists. split.
      + apply msteps_one. exact (match_header _ _ _ _ _ _ _ Hf Hr H1 H2 Hlj).
      + constructor. destruct Hm. constructor; simpl; auto. econstructor; eauto.
    - (* match, unpacking fields *) body_code Hl. apply match_inv in He as (blocks & Hb & ->).
      apply code_at_cons in Hcode as [Hf Hcode].
      destruct (emit_cases_nth _ _ _ _ _ _ _ _ _ _ Hb Hcode H3) as (l & code & Hlj & Hcl & [[Hn _] | [_ (cb & Hcb & ->)]]);
        [lia |].
      destruct (arities_cases_nth AsmEmit.arities_expr b cases 0 _ _ _ _ H3 Hinc) as [Hib Hpair].
      pose proof (ms_regs _ _ Hm r) as Hr. rewrite H0 in Hr. apply wr_con_inv in Hr.
      destruct (hget_rel_some _ _ _ _ _ (ms_heap _ _ Hm) H5) as (o2 & Hg2 & Ho).
      inversion Ho as [| ws1 ws2 Hws |]; subst.
      assert (Har : ar (b + (0 + (t - b))) = n) by (apply arity_of_In; [exact Hcons | apply Hpair; exact H4]).
      apply code_at_cons in Hcl as [Hfu Hcb'].
      eexists. split.
      + eapply msteps_more; [exact (match_header _ _ _ _ _ _ _ Hf Hr H1 H2 Hlj) |].
        apply msteps_one. eapply Machine.s_unpack; simpl.
        * exact Hfu.
        * fold ar. rewrite Har. exact H4.
        * exact Hr.
        * exact Hg2.
        * fold ar. rewrite Har, <- (Forall2_length Hws). exact H6.
      + constructor. destruct Hm. constructor; simpl; auto.
        * econstructor; eauto.
        * replace (ar (b + (t - b))) with n by (rewrite <- Har; reflexivity).
          apply upd_from_rel; [assumption | apply Forall2_firstn; exact Hws].
    - (* fin *) body_code Hl. cbn [AsmEmit.emit_expr] in He. injection He as <-.
      apply code_at_cons in Hcode as [Hf _].
      eexists. split; [apply msteps_one; apply Machine.s_fin; exact Hf |].
      rewrite (ms_hp _ _ Hm). constructor; [constructor; apply (ms_regs _ _ Hm) | exact (ms_heap _ _ Hm)].
    - (* return stub *) rewrite H in Hl. assert (Hpc : Machine.pc s2 = 0) by (inversion Hl; auto).
      eexists. split.
      + apply msteps_one. apply Machine.s_fin. unfold Machine.fetch. rewrite Hpc. exact Hret.
      + rewrite (ms_hp _ _ Hm). constructor; [constructor; apply (ms_regs _ _ Hm) | exact (ms_heap _ _ Hm)].
    - (* extern stub *) rewrite H in Hl.
      assert (Hsa : sa j = Some (Machine.pc s2)) by (inversion Hl; subst; assumption).
      pose proof (Hstubs _ _ Hsa) as Hsc. unfold AsmEmit.stub in Hsc.
      apply code_at_cons in Hsc as [Hx Hsc]. apply code_at_cons in Hsc as [Hmv Hsc].
      apply code_at_cons in Hsc as [Hen _].
      destruct (Hh _ _ lab _ _ _ _ _ _ _ _ _ (ms_regs _ _ Hm A1) (ms_regs _ _ Hm) (ms_heap _ _ Hm) H0)
        as (res2 & Hh2 & Hres).
      rewrite (ms_hp _ _ Hm) in Hh2.
      inversion Hres as [w1 w2 R1' R2' H1' H2' hp0 Hw HR' HH' | ]; subst.
      assert (Hc : word_rel lab (AsmSem.stub_regs R' w CONT)
                     (upd (upd R2' X01 w2) A1 (upd R2' X01 w2 X01) CONT)) by exact (HR' CONT).
      destruct (code_of_rel _ _ _ _ _ HH' Hc H1) as (l2 & Hc2 & Hl2).
      destruct (lab_code _ _ Hl2 H2) as (i2 & Hi2).
      eexists. split.
      + eapply msteps_more; [eapply Machine.s_extern; [exact Hx | exact Hh2] |].
        eapply msteps_more; [apply Machine.s_mov; exact Hmv |].
        apply msteps_one. eapply Machine.s_encore; [exact Hen | exact Hc2 | exact Hi2].
      + constructor. constructor; simpl.
        * exact Hl2.
        * unfold AsmSem.call_regs. cbn [upd_from map].
          apply upd_rel; [apply upd_rel |].
          -- unfold AsmSem.stub_regs. apply upd_rel; [apply upd_rel; [exact HR' | exact Hw] | exact Hw].
          -- exact Hc.
          -- exact (HR' NULL).
        * exact HH'.
        * reflexivity.
        * exact (ms_glob _ _ Hm).
    - (* extern stub resuming NULL *) rewrite H in Hl.
      assert (Hsa : sa j = Some (Machine.pc s2)) by (inversion Hl; subst; assumption).
      pose proof (Hstubs _ _ Hsa) as Hsc. unfold AsmEmit.stub in Hsc.
      apply code_at_cons in Hsc as [Hx Hsc]. apply code_at_cons in Hsc as [Hmv Hsc].
      apply code_at_cons in Hsc as [Hen _].
      destruct (Hh _ _ lab _ _ _ _ _ _ _ _ _ (ms_regs _ _ Hm A1) (ms_regs _ _ Hm) (ms_heap _ _ Hm) H0)
        as (res2 & Hh2 & Hres).
      rewrite (ms_hp _ _ Hm) in Hh2.
      inversion Hres as [w1 w2 R1' R2' H1' H2' hp0 Hw HR' HH' | ]; subst.
      assert (Hc : word_rel lab (AsmSem.stub_regs R' w CONT)
                     (upd (upd R2' X01 w2) A1 (upd R2' X01 w2 X01) CONT)) by exact (HR' CONT).
      destruct (code_of_rel _ _ _ _ _ HH' Hc H1) as (l2 & Hc2 & Hl2). inversion Hl2; subst.
      eexists. split.
      + eapply msteps_more; [eapply Machine.s_extern; [exact Hx | exact Hh2] |].
        eapply msteps_more; [apply Machine.s_mov; exact Hmv |].
        apply msteps_one. eapply Machine.s_encore_outside; [exact Hen | exact Hc2 | exact null_outside].
      + constructor; [constructor; reflexivity | exact HH'].
    - (* extern failure *) rewrite H in Hl.
      assert (Hsa : sa j = Some (Machine.pc s2)) by (inversion Hl; subst; assumption).
      pose proof (Hstubs _ _ Hsa) as Hsc. unfold AsmEmit.stub in Hsc.
      apply code_at_cons in Hsc as [Hx _].
      destruct (Hh _ _ lab _ _ _ _ _ _ _ _ _ (ms_regs _ _ Hm A1) (ms_regs _ _ Hm) (ms_heap _ _ Hm) H0)
        as (res2 & Hh2 & Hres).
      rewrite (ms_hp _ _ Hm) in Hh2.
      inversion Hres as [| H1' H2' hp0 HH']; subst.
      eexists. split.
      + apply msteps_one. eapply Machine.s_extern_fail; [exact Hx | exact Hh2].
      + constructor; [constructor; reflexivity | exact HH'].
  Qed.

  (** ** Whole runs *)

  Definition mexec := Machine.exec C ar mem entries (h Machine.label).
  Definition aexec := AsmSem.exec mem prog (h AsmSem.alabel).

  Lemma msteps_exec s r o H2 hp :
    msteps s r ->
    (r = Machine.Stop o H2 hp \/ exists s', r = Machine.Run s' /\ mexec s' o H2 hp) ->
    mexec s o H2 hp.
  Proof.
    induction 1 as [s r Hs | s s' r Hs _ IH]; intros [-> | (s'' & -> & He)].
    - apply Machine.exec_stop. exact Hs.
    - eapply Machine.exec_run; [exact Hs | exact He].
    - eapply Machine.exec_run; [exact Hs | apply IH; left; reflexivity].
    - eapply Machine.exec_run; [exact Hs | apply IH; right; eauto].
  Qed.

  Lemma exec_sim s1 o1 H1 hp :
    aexec s1 o1 H1 hp -> forall s2, match_states s1 s2 ->
    exists o2 H2, mexec s2 o2 H2 hp /\ outcome_rel lab o1 o2 /\ heap_rel lab H1 H2.
  Proof.
    induction 1 as [s1 o1 H1 hp Hs | s1 s1' o1 H1 hp Hs _ IH]; intros s2 Hm.
    - destruct (step_sim _ _ _ Hs Hm) as (r2 & Hms & Hr). inversion Hr as [| ? o2 ? H2 ? Ho HH]; subst.
      exists o2, H2. split; [eapply msteps_exec; [exact Hms | left; reflexivity] | auto].
    - destruct (step_sim _ _ _ Hs Hm) as (r2 & Hms & Hr). inversion Hr as [? s2' Hm' | ]; subst.
      destruct (IH s2' Hm') as (o2 & H2 & He & Ho & HH).
      exists o2, H2. split; [eapply msteps_exec; [exact Hms | right; eauto] | auto].
  Qed.

  (** The program's defines and their entry points. *)
  Hypothesis Hdefs : Forall2 (fun d l => lab (AsmSem.Body (Asm.def_body d)) l) prog entries.

  Definition vm_rel (m1 : AsmSem.vm) (m2 : Machine.vm) : Prop :=
    heap_rel lab (AsmSem.vheap m1) (Machine.vheap m2) /\ AsmSem.vhp m1 = Machine.vhp m2 /\
    Forall2 (word_rel lab) (AsmSem.vglobals m1) (Machine.vglobals m2).

  Inductive err_rel : option (error AsmSem.alabel) -> option (error Machine.label) -> Prop :=
  | err_none : err_rel None None
  | err_some e1 e2 : kind e1 = kind e2 -> err_rel (Some e1) (Some e2).

  Lemma entry_regs_rel l1 l2 :
    lab l1 l2 ->
    regs_rel lab (AsmSem.entry_regs (WFunc l1) [AsmSem.thunk_arg])
                 (Machine.entry_regs (WFunc l2) [Machine.thunk_arg]).
  Proof.
    intros Hl. unfold AsmSem.entry_regs, Machine.entry_regs.
    apply upd_from_rel; [| repeat constructor].
    apply upd_rel; [apply upd_rel |].
    - apply clear_regs_rel. constructor.
    - constructor. exact Hl.
    - constructor. constructor.
  Qed.

  Lemma call_define_sim m1 m2 body l o1 m1' :
    AsmSem.call_define mem prog (h AsmSem.alabel) m1 body o1 m1' -> vm_rel m1 m2 ->
    lab (AsmSem.Body body) l ->
    exists o2 m2', Machine.call_address C ar mem entries (h Machine.label) m2 l o2 m2' /\
                   outcome_rel lab o1 o2 /\ vm_rel m1' m2'.
  Proof.
    intros Hc [HH [Hhp HG]] Hl. inversion Hc as [o H1' hp' He]; subst.
    assert (Hm : match_states
                   (AsmSem.St (AsmSem.Body body) (AsmSem.entry_regs (WFunc (AsmSem.Body body)) [AsmSem.thunk_arg])
                              (AsmSem.vheap m1) (AsmSem.vhp m1) (AsmSem.vglobals m1))
                   (Machine.St l (Machine.entry_regs (WFunc l) [Machine.thunk_arg])
                               (Machine.vheap m2) (Machine.vhp m2) (Machine.vglobals m2))).
    { constructor; simpl; auto. apply entry_regs_rel. exact Hl. }
    destruct (exec_sim _ _ _ _ He _ Hm) as (o2 & H2' & He2 & Ho & HH').
    exists o2, (Machine.Vm H2' hp' (Machine.vglobals m2)). split; [| split; [exact Ho |]].
    - apply Machine.call_address_run. exact He2.
    - split; [exact HH' | split; [reflexivity | exact HG]].
  Qed.

  Lemma Forall2_skipn {A B} (P : A -> B -> Prop) n l1 l2 :
    Forall2 P l1 l2 -> Forall2 P (skipn n l1) (skipn n l2).
  Proof.
    intros H. revert n. induction H as [| x y l1 l2 Hxy Hl IH]; intros [| n]; simpl; try constructor; auto.
  Qed.

  Lemma set_global_rel m1 m2 i w1 w2 :
    vm_rel m1 m2 -> word_rel lab w1 w2 ->
    vm_rel (AsmSem.set_global m1 i w1) (Machine.set_global m2 i w2).
  Proof.
    intros [HH [Hhp HG]] Hw. unfold AsmSem.set_global, Machine.set_global, vm_rel. simpl.
    split; [exact HH | split; [exact Hhp |]].
    apply Forall2_app; [apply Forall2_firstn; exact HG | constructor; [exact Hw | exact (Forall2_skipn _ (S i) _ _ HG)]].
  Qed.

  Lemma load_defines_sim i ds m1 r1 m1' :
    AsmSem.load_defines mem prog (h AsmSem.alabel) i ds m1 r1 m1' ->
    forall es m2, Forall2 (fun d l => lab (AsmSem.Body (Asm.def_body d)) l) ds es -> vm_rel m1 m2 ->
    exists r2 m2', Machine.load_defines C ar mem entries (h Machine.label) i es m2 r2 m2' /\
                   err_rel r1 r2 /\ vm_rel m1' m2'.
  Proof.
    induction 1 as [i m1 | i d ds m1 w m1' r m1'' Hc _ IH | i d ds m1 e m1' Hc];
      intros es m2 Hds Hm.
    - inversion Hds; subst. exists None, m2. split; [apply Machine.load_done | split; [constructor | exact Hm]].
    - inversion Hds as [| ? l ? es' Hl Hds']; subst.
      destruct (call_define_sim _ _ _ _ _ _ Hc Hm Hl) as (o2 & m2' & Hc2 & Ho & Hm').
      inversion Ho as [? w2 Hw |]; subst.
      destruct (IH es' _ Hds' (set_global_rel _ _ i _ _ Hm' Hw)) as (r2 & m2'' & Hl2 & Hr & Hm'').
      exists r2, m2''. split; [eapply Machine.load_next; [exact Hc2 | exact Hl2] | auto].
    - inversion Hds as [| ? l ? es' Hl Hds']; subst.
      destruct (call_define_sim _ _ _ _ _ _ Hc Hm Hl) as (o2 & m2' & Hc2 & Ho & Hm').
      inversion Ho as [| ? e2 Hk]; subst.
      exists (Some e2), m2'. split; [eapply Machine.load_fail; exact Hc2 | split; [constructor; exact Hk | exact Hm']].
  Qed.

  Lemma repeat_rel n : Forall2 (word_rel lab) (repeat (WInt 0) n) (repeat (WInt 0) n).
  Proof. induction n; simpl; constructor; auto. constructor. Qed.

  Lemma load_sim r1 m1 :
    AsmSem.load mem prog (h AsmSem.alabel) r1 m1 ->
    exists r2 m2, Machine.load C ar mem entries (h Machine.label) r2 m2 /\ err_rel r1 r2 /\ vm_rel m1 m2.
  Proof.
    destruct 1 as [Hlt | r m Hle Hl].
    - exists (Some (GlobalsOverflow (length entries) mem)), Machine.init. split.
      + apply Machine.load_overflow. rewrite Hent. exact Hlt.
      + split; [constructor; simpl; rewrite Hent; reflexivity |].
        split; [constructor | split; [reflexivity | constructor]].
    - destruct (load_defines_sim _ _ _ _ _ Hl entries (Machine.Vm [] 0 (repeat (WInt 0) (length entries))) Hdefs)
        as (r2 & m2 & Hl2 & Hr & Hm).
      + split; [constructor | split; [reflexivity |]]. simpl. rewrite Hent. apply repeat_rel.
      + exists r2, m2. split; [| auto]. apply Machine.load_run; [rewrite Hent; exact Hle | exact Hl2].
  Qed.

  Lemma observe_globals_rel H1 H2 G1 G2 gs :
    heap_rel lab H1 H2 -> Forall2 (word_rel lab) G1 G2 ->
    Forall2 (fun w o => observe H1 (Halt w) o) G1 gs -> Forall2 (fun w o => observe H2 (Halt w) o) G2 gs.
  Proof.
    intros HH HG Hob. revert G2 HG. induction Hob as [| w o G1 gs Ho _ IH]; intros G2 HG;
      inversion HG as [| ? w2 ? G2' Hw HG']; subst; constructor.
    - eapply observe_rel; [exact HH | constructor; exact Hw | exact Ho].
    - apply IH. exact HG'.
  Qed.

  Lemma behaves_sim lo :
    AsmSem.behaves (h AsmSem.alabel) prog mem lo ->
    Machine.behaves (h Machine.label) (Machine.Program C ar entries) mem lo.
  Proof.
    destruct 1 as [m gs Hl Hob | e m Hl].
    - destruct (load_sim _ _ Hl) as (r2 & m2 & Hl2 & Hr & [HH [_ HG]]). inversion Hr; subst.
      eapply Machine.behaves_loaded; [exact Hl2 |]. eapply observe_globals_rel; eauto.
    - destruct (load_sim _ _ Hl) as (r2 & m2 & Hl2 & Hr & _). inversion Hr as [| ? e2 Hk]; subst.
      rewrite Hk. eapply Machine.behaves_trap. exact Hl2.
  Qed.
End Sim.
