(** Semantics of the [asm] IR (theories/ir/Asm.v), on the run-time
    structures of Runtime.v. An asm program means what [asm_emit] makes of
    it on the VM (Machine.v), with two deliberate exceptions, chosen so
    that the meaning of asm is the clean one and each lowering trick of the
    emitter is checked or proved in one place (Emit.v):

    - [encore f (x₁ … xₙ) k] is a parallel assignment: [A1 … An], [SELF]
      and [CONT] all receive the values the registers held before the
      call. The emitter moves arguments one [MOV] at a time, which agrees
      only when no move overwrites a register read later; Emit.v checks
      that condition ([no_clobber]).
    - A [match] whose tag has no case is undefined behaviour: there is no
      rule, the program is stuck, so the emitter owes it nothing. The
      emitter's [BRANCH] (any tag other than [base] goes to the second
      case) is then a correct lowering. Only the compiler produces asm, and
      its matches never see an out-of-range tag.

    Everything else follows Machine.v: the same register file, heap,
    allocation, collection and traps. In particular the extern stub writes
    the scratch register [X01], as the emitted stub does, since every
    register is a collection root.

    A code address is a piece of the program: the body of a function or
    continuation, an extern stub, the return stub, or the [NULL] register's
    sentinel, which is no code at all. *)

From Stdlib Require Import String Strings.Byte List ZArith NArith PeanoNat Bool.
Import ListNotations.
From EncorePasses Require Import Runtime.
From EncorePasses Require Asm Prim.

Inductive alabel : Type :=
| Body (e : Asm.expr)        (** a function or continuation body *)
| Stub (slot : nat)          (** the stub calling extern [slot] *)
| Return                     (** the return stub, [fin a1] *)
| NullCode.                  (** the [NULL] sentinel: calling it traps *)

Notation word := (Runtime.word alabel).
Notation obj := (Runtime.obj alabel).
Notation heap := (Runtime.heap alabel).
Notation regs := (Runtime.regs alabel).
Notation error := (Runtime.error alabel).
Notation outcome := (Runtime.outcome alabel).
Notation host_result := (Runtime.host_result alabel).

Definition return_cont : word := WFunc Return.
Definition thunk_arg : word := WInt 0.

(** Integer operations of the asm IR that are binary operations of the VM. *)
Definition int_binop (o : Prim.int_op) : option binop :=
  match o with
  | Prim.Add => Some Add | Prim.Sub => Some Sub | Prim.Mul => Some Mul
  | Prim.Eq => Some Eq | Prim.Lt => Some Lt | Prim.Le => Some Le
  | Prim.Div => Some Div | Prim.Mod => Some Mod | Prim.SubSat => Some SubSat
  | Prim.And => Some And | Prim.Or => Some Or | Prim.Xor => Some Xor
  | Prim.Shl => Some Shl | Prim.Shr => Some Shr
  | Prim.Byte => None
  end.

Record state : Type := St {
  ctl : alabel;             (** what runs next: the rest of a body, or a stub *)
  R : regs;
  H : heap;
  hp : nat;
  G : list word
}.

Inductive step_result : Type :=
| Run (s : state)
| Stop (o : outcome) (H : heap) (hp : nat).

Definition as_int (w : word) : option Z :=
  match w with WInt n => Some n | _ => None end.

Definition code_of (Hp : heap) (w : word) : option alabel :=
  match w with
  | WFunc l => Some l
  | WClos a => match hget Hp a with Some (OClosure l _) => Some l | _ => None end
  | _ => None
  end.

(** The registers after the extern stub's [EXTERN X01 A1 j; MOV A1 X01]. *)
Definition stub_regs (R' : regs) (w : word) : regs := upd (upd R' X01 w) A1 w.

(** [encore]'s parallel assignment, before the jump. *)
Definition call_regs (Rg : regs) (f : nat) (args : list nat) (k : nat) : regs :=
  upd (upd (upd_from Rg A1 (map Rg args)) SELF (Rg f)) CONT (Rg k).

Section Asm.
  Variable mem_words : nat.
  Variable prog : Asm.module.
  Variable host : nat -> word -> regs -> heap -> nat -> host_result -> Prop.

  Definition cap : nat := mem_words - length prog.

  Definition goto (s : state) (e : Asm.expr) (R' : regs) : step_result :=
    Run (St (Body e) R' (H s) (hp s) (G s)).

  Definition trap (s : state) (e : error) : step_result := Stop (Trap e) (H s) (hp s).

  Definition bytes_of (s : state) (w : word) : option (list Byte.byte) :=
    match w with
    | WBytes a => match hget (H s) a with Some (OBytes b) => Some b | _ => None end
    | _ => None
    end.

  (** Allocate [o], put [mk a] in [d] and continue with [e], or trap. *)
  Inductive allocate (s : state) (o : obj) (d : nat) (mk : addr -> word) (e : Asm.expr)
      : step_result -> Prop :=
  | allocate_ok H' hp' a :
      alloc cap (R s) (G s) (H s) (hp s) o (Allocated H' hp' a) ->
      allocate s o d mk e (Run (St (Body e) (upd (R s) d (mk a)) H' hp' (G s)))
  | allocate_oom H' hp' :
      alloc cap (R s) (G s) (H s) (hp s) o (OutOfMemory H' hp') ->
      allocate s o d mk e (Stop (Trap HeapOverflow) H' hp').

  (** A function or continuation value: a function when nothing is
      captured ([FUNCTION]), else a closure ([CLOSURE]). *)
  Inductive make_closure (s : state) (d : nat) (caps : list nat) (body e : Asm.expr)
      : step_result -> Prop :=
  | make_function :
      caps = [] -> make_closure s d caps body e (goto s e (upd (R s) d (WFunc (Body body))))
  | make_closure_alloc res :
      caps <> [] -> allocate s (OClosure (Body body) (map (R s) caps)) d WClos e res ->
      make_closure s d caps body e res.

  Inductive step (s : state) : step_result -> Prop :=
  (* let: one rule per kind of value *)
  | s_reg d r e :
      ctl s = Body (Asm.Let d (Asm.Reg r) e) -> step s (goto s e (upd (R s) d (R s r)))
  | s_capture d i e a l env w :
      ctl s = Body (Asm.Let d (Asm.Capture i) e) ->
      R s SELF = WClos a -> hget (H s) a = Some (OClosure l env) -> nth_error env i = Some w ->
      step s (goto s e (upd (R s) d w))
  | s_global d i e w :
      ctl s = Body (Asm.Let d (Asm.Global i) e) -> nth_error (G s) i = Some w ->
      step s (goto s e (upd (R s) d w))
  | s_cont_lam d caps body e res :
      ctl s = Body (Asm.Let d (Asm.ContLam caps body) e) ->
      make_closure s d caps body e res -> step s res
  | s_ctor_nullary d t e :
      ctl s = Body (Asm.Let d (Asm.Ctor t []) e) -> step s (goto s e (upd (R s) d (WCon t None)))
  | s_ctor d t fs e res :
      ctl s = Body (Asm.Let d (Asm.Ctor t fs) e) -> fs <> [] ->
      allocate s (OFields (map (R s) fs)) d (fun a => WCon t (Some a)) e res -> step s res
  | s_field d r i e t a ws w :
      ctl s = Body (Asm.Let d (Asm.Field r i) e) ->
      R s r = WCon t (Some a) -> hget (H s) a = Some (OFields ws) -> nth_error ws i = Some w ->
      step s (goto s e (upd (R s) d w))
  | s_int d n e :
      ctl s = Body (Asm.Let d (Asm.VInt n) e) -> step s (goto s e (upd (R s) d (WInt n)))
  | s_bytes d b e res :
      ctl s = Body (Asm.Let d (Asm.Bytes b) e) ->
      allocate s (OBytes (list_byte_of_string b)) d WBytes e res -> step s res
  | s_extern d j e :
      ctl s = Body (Asm.Let d (Asm.Extern j) e) -> step s (goto s e (upd (R s) d (WFunc (Stub j))))
  (* primitives *)
  | s_binop d o b x y e n m r :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PInt o) [x; y]) e) -> int_binop o = Some b ->
      as_int (R s x) = Some n -> as_int (R s y) = Some m -> binop_sem b n m = Some r ->
      step s (goto s e (upd (R s) d (prim_word r)))
  | s_binop_overflow d o b x y e n m :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PInt o) [x; y]) e) -> int_binop o = Some b ->
      as_int (R s x) = Some n -> as_int (R s y) = Some m -> binop_sem b n m = None ->
      step s (trap s (IntOverflow (ctl s)))
  | s_binop_type d o b x y e :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PInt o) [x; y]) e) -> int_binop o = Some b ->
      as_int (R s x) = None \/ as_int (R s y) = None -> step s (trap s TypeError)
  | s_int_byte d x e n c res :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PInt Prim.Byte) [x]) e) ->
      R s x = WInt n -> (0 <= n)%Z -> Byte.of_N (Z.to_N n) = Some c ->
      allocate s (OBytes [c]) d WBytes e res -> step s res
  | s_int_byte_range d x e n :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PInt Prim.Byte) [x]) e) ->
      R s x = WInt n -> (n < 0 \/ 255 < n)%Z -> step s (trap s (ByteRange n (ctl s)))
  | s_int_byte_type d x e :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PInt Prim.Byte) [x]) e) ->
      as_int (R s x) = None -> step s (trap s TypeError)
  | s_bytes_len d x e b :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PBytes Prim.Len) [x]) e) -> bytes_of s (R s x) = Some b ->
      step s (goto s e (upd (R s) d (WInt (Z.of_nat (length b)))))
  | s_bytes_get d x i e n b c :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PBytes Prim.Get) [x; i]) e) ->
      R s i = WInt n -> bytes_of s (R s x) = Some b -> (0 <= n)%Z -> nth_error b (Z.to_nat n) = Some c ->
      step s (goto s e (upd (R s) d (WInt (Z.of_N (Byte.to_N c)))))
  | s_bytes_get_type d x i e :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PBytes Prim.Get) [x; i]) e) ->
      as_int (R s i) = None -> step s (trap s TypeError)
  | s_bytes_concat d x y e b1 b2 res :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PBytes Prim.Concat) [x; y]) e) ->
      bytes_of s (R s x) = Some b1 -> bytes_of s (R s y) = Some b2 ->
      allocate s (OBytes (b1 ++ b2)) d WBytes e res -> step s res
  | s_bytes_slice d x i n e st len b res :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PBytes Prim.Slice) [x; i; n]) e) ->
      R s i = WInt st -> R s n = WInt len -> (0 <= st)%Z -> (0 <= len)%Z ->
      bytes_of s (R s x) = Some b -> Z.to_nat st + Z.to_nat len <= length b ->
      allocate s (OBytes (firstn (Z.to_nat len) (skipn (Z.to_nat st) b))) d WBytes e res ->
      step s res
  | s_bytes_slice_oom d x i n e st len H' hp' :
      (* the VM allocates before it reads the source *)
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PBytes Prim.Slice) [x; i; n]) e) ->
      R s i = WInt st -> R s n = WInt len -> (0 <= len)%Z ->
      alloc cap (R s) (G s) (H s) (hp s) (OBytes (repeat Byte.x00 (Z.to_nat len))) (OutOfMemory H' hp') ->
      step s (Stop (Trap HeapOverflow) H' hp')
  | s_bytes_slice_type d x i n e :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PBytes Prim.Slice) [x; i; n]) e) ->
      as_int (R s i) = None \/ as_int (R s n) = None -> step s (trap s TypeError)
  | s_bytes_eq d x y e b1 b2 :
      ctl s = Body (Asm.Let d (Asm.Prim (Prim.PBytes Prim.BEq) [x; y]) e) ->
      bytes_of s (R s x) = Some b1 -> bytes_of s (R s y) = Some b2 ->
      step s (goto s e (upd (R s) d
        (bool_word (if list_eq_dec Byte.byte_eq_dec b1 b2 then true else false))))
  (* the other expressions *)
  | s_letrec d caps body e res :
      ctl s = Body (Asm.Letrec d caps body e) -> make_closure s d caps body e res -> step s res
  | s_encore f args k l :
      ctl s = Body (Asm.Encore f args k) -> code_of (H s) (R s f) = Some l -> l <> NullCode ->
      step s (Run (St l (call_regs (R s) f args k) (H s) (hp s) (G s)))
  | s_encore_null f args k :
      ctl s = Body (Asm.Encore f args k) -> code_of (H s) (R s f) = Some NullCode ->
      step s (trap s (CallOutsideCode (ctl s)))
  | s_match r b cases t oa ub e :
      (* a tag with no case has no rule: undefined behaviour *)
      ctl s = Body (Asm.Match r b cases) -> R s r = WCon t oa -> b <= t ->
      nth_error cases (t - b) = Some (0, ub, e) ->
      step s (goto s e (R s))
  | s_match_unpack r b cases t a n ub e ws :
      ctl s = Body (Asm.Match r b cases) -> R s r = WCon t (Some a) -> b <= t ->
      nth_error cases (t - b) = Some (n, ub, e) -> n > 0 ->
      hget (H s) a = Some (OFields ws) -> n <= length ws ->
      step s (goto s e (upd_from (R s) ub (firstn n ws)))
  | s_fin r :
      ctl s = Body (Asm.Fin r) -> step s (Stop (Halt (R s r)) (H s) (hp s))
  (* the stubs the emitter adds *)
  | s_return :
      ctl s = Return -> step s (Stop (Halt (R s A1)) (H s) (hp s))
  | s_stub j w R' H' hp' l :
      ctl s = Stub j -> host j (R s A1) (R s) (H s) (hp s) (HostOk w R' H' hp') ->
      code_of H' (stub_regs R' w CONT) = Some l -> l <> NullCode ->
      step s (Run (St l (call_regs (stub_regs R' w) CONT [] NULL) H' hp' (G s)))
  | s_stub_null j w R' H' hp' :
      ctl s = Stub j -> host j (R s A1) (R s) (H s) (hp s) (HostOk w R' H' hp') ->
      code_of H' (stub_regs R' w CONT) = Some NullCode ->
      step s (Stop (Trap (CallOutsideCode (ctl s))) H' hp')
  | s_stub_fail j H' hp' :
      ctl s = Stub j -> host j (R s A1) (R s) (H s) (hp s) (HostErr H' hp') ->
      step s (Stop (Trap (ExternFail j (ctl s))) H' hp').

  (** ** Running to completion, loading, calling: as in Machine.v *)

  Inductive exec : state -> outcome -> heap -> nat -> Prop :=
  | exec_stop s o H' hp' : step s (Stop o H' hp') -> exec s o H' hp'
  | exec_run s s' o H' hp' : step s (Run s') -> exec s' o H' hp' -> exec s o H' hp'.

  Record vm : Type := Vm { vheap : heap; vhp : nat; vglobals : list word }.

  Definition entry_regs (f : word) (args : list word) : regs :=
    upd_from (upd (upd (clear_regs NullCode) SELF f) CONT return_cont) A1 args.

  (** A define's thunk: its body runs with itself in [SELF]. *)
  Inductive call_define (m : vm) (body : Asm.expr) : outcome -> vm -> Prop :=
  | call_define_run o H' hp' :
      exec (St (Body body) (entry_regs (WFunc (Body body)) [thunk_arg]) (vheap m) (vhp m) (vglobals m))
        o H' hp' ->
      call_define m body o (Vm H' hp' (vglobals m)).

  Definition set_global (m : vm) (i : nat) (w : word) : vm :=
    Vm (vheap m) (vhp m) (firstn i (vglobals m) ++ w :: skipn (S i) (vglobals m)).

  (** Defines run in module order, and define [i] fills global [i], as in
      the emitted program; [def_global] is not consulted. *)
  Inductive load_defines : nat -> list Asm.define -> vm -> option error -> vm -> Prop :=
  | load_done i m : load_defines i [] m None m
  | load_next i d ds m w m' r m'' :
      call_define m (Asm.def_body d) (Halt w) m' ->
      load_defines (S i) ds (set_global m' i w) r m'' ->
      load_defines i (d :: ds) m r m''
  | load_fail i d ds m e m' :
      call_define m (Asm.def_body d) (Trap e) m' -> load_defines i (d :: ds) m (Some e) m'.

  Inductive load : option error -> vm -> Prop :=
  | load_overflow :
      mem_words < length prog ->
      load (Some (GlobalsOverflow (length prog) mem_words)) (Vm [] 0 [])
  | load_run r m :
      length prog <= mem_words ->
      load_defines 0 prog (Vm [] 0 (repeat (WInt 0) (length prog))) r m -> load r m.
End Asm.

(** [behaves host p mem lo]: loading asm program [p] in a buffer of [mem]
    words is observed as [lo], with the same observations as Machine.v. *)
Inductive behaves (host : nat -> word -> regs -> heap -> nat -> host_result -> Prop)
    (p : Asm.module) (mem : nat) : load_obs -> Prop :=
| behaves_loaded m gs :
    load mem p host None m ->
    Forall2 (fun w o => observe (vheap m) (Halt w) o) (vglobals m) gs ->
    behaves host p mem (Loaded gs)
| behaves_trap e m :
    load mem p host (Some e) m -> behaves host p mem (LoadTrap (kind e)).
