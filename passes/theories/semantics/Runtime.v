(** The run-time structures shared by every machine-level semantics:
    words, heap objects, registers, allocation, garbage collection,
    primitives, errors, and what an outcome lets an observer see.
    Transcribed from crates/encore_vm/src (value.rs, arena.rs, gc.rs,
    registers.rs, int.rs, error.rs).

    Everything is parameterized by [label], the type of code addresses: an
    instruction index for the bytecode machine (Machine.v), a piece of the
    program for a higher-level IR. A code address appears in function and
    closure values and in error positions, and nowhere else, so the heap,
    the collector and the primitives are the same for every language that
    instantiates this module.

    Assumption: the garbage collector is correct. [collect] specifies what
    gc.rs must do (keep exactly the reachable objects, up to renaming of
    addresses, and reset the bump pointer to their total size). That gc.rs
    implements this specification is trusted, not proved. In particular,
    this module gives every object its intended size, while the VM
    currently corrupts the GC header of objects of 128 words or more, i.e.
    byte strings over 500 bytes built at run time (issue #28). *)

From Stdlib Require Import List ZArith NArith PeanoNat Bool Strings.Byte.
Import ListNotations.

Definition reg := nat.
Definition addr := nat.
Definition tag := nat.

(** ** Registers and integers (value.rs, registers.rs) *)

Definition SELF : reg := 0.
Definition CONT : reg := 1.
Definition A1 : reg := 2.
Definition NULL : reg := 255.
(** The first local register; the compiler's extern stubs use it as scratch. *)
Definition X01 : reg := 10.
Definition n_regs : nat := 256.

Definition int_min : Z := (- 2 ^ 23)%Z.
Definition int_max : Z := (2 ^ 23 - 1)%Z.
Definition in_range (n : Z) : bool := (int_min <=? n)%Z && (n <=? int_max)%Z.

(** ** Integer operations (int.rs), independent of code addresses *)

Inductive binop : Type :=
| Add | Sub | Mul | Eq | Lt | Le | Div | Mod | SubSat | And | Or | Xor | Shl | Shr.

Definition checked (n : Z) : option Z := if in_range n then Some n else None.

(** [Value::int] keeps the low 24 bits and [int_value] sign-extends them. *)
Definition wrap24 (n : Z) : Z :=
  let m := (n mod 2 ^ 24)%Z in if (m <=? int_max)%Z then m else (m - 2 ^ 24)%Z.

(** The result of a binary operation: an integer, a boolean (a nullary
    constructor of tag 0 or 1), or [None] for [IntOverflow]. *)
Inductive prim_result : Type :=
| PInt (n : Z)
| PBool (b : bool).

Definition binop_sem (o : binop) (a b : Z) : option prim_result :=
  match o with
  | Add => option_map PInt (checked (a + b))
  | Sub => option_map PInt (checked (a - b))
  | Mul => option_map PInt (checked (a * b))
  | Eq => Some (PBool (a =? b)%Z)
  | Lt => Some (PBool (a <? b)%Z)
  | Le => Some (PBool (a <=? b)%Z)
  | Div => if (b =? 0)%Z then Some (PInt 0) else option_map PInt (checked (Z.quot a b))
  | Mod => Some (PInt (if (b =? 0)%Z then a else Z.rem a b))
  | SubSat => if (a <=? b)%Z then Some (PInt 0) else option_map PInt (checked (a - b))
  | And => Some (PInt (Z.land a b))
  | Or => Some (PInt (Z.lor a b))
  | Xor => Some (PInt (Z.lxor a b))
  | Shl =>
      if (b <? 0)%Z || (a =? 0)%Z then Some (PInt 0)
      else if (24 <=? b)%Z then None
      else option_map PInt (checked (a * 2 ^ b))
  | Shr =>
      if (b <? 0)%Z || (24 <=? b)%Z then Some (PInt 0)
      else Some (PInt (wrap24 (Z.shiftr (a mod 2 ^ 24) b)))
  end.

(** ** Observable values *)

(** First-order values: what a result means independently of heap
    addresses and code. This is the [≈_H] relation of the paper. *)
Inductive value : Type :=
| VInt (n : Z)
| VBytes (s : list Byte.byte)
| VCon (t : tag) (fields : list value).

(** A trap with its code position erased: positions differ between
    languages, the reason does not. *)
Inductive trap_kind : Type :=
| KHeapOverflow
| KMatchFail (t : tag)
| KByteRange (v : Z)
| KIntOverflow
| KExternFail (slot : nat)
| KTypeError
| KCallOutsideCode
| KGlobalOutOfRange
| KGlobalsOverflow (needed available : nat).

(** What an observer sees of a finished run: a first-order result, a
    result that is (or contains) a function, or a trap. *)
Inductive obs : Type :=
| OReturn (v : value)
| OHigher
| OTrap (k : trap_kind).

(** [o2] is an acceptable target behaviour for source behaviour [o1]: the
    same observation, or running out of memory, since a transformation may
    change how much a program allocates. *)
Definition refines (o1 o2 : obs) : Prop := o2 = o1 \/ o2 = OTrap KHeapOverflow.

(** What an observer sees of loading a program: the value of each define,
    or the trap that stopped the load. *)
Inductive load_obs : Type :=
| Loaded (globals : list obs)
| LoadTrap (k : trap_kind).

Definition load_refines (l1 l2 : load_obs) : Prop :=
  l2 = l1 \/ l2 = LoadTrap KHeapOverflow.

Lemma load_refines_refl : forall l, load_refines l l.
Proof. now left. Qed.

Lemma load_refines_trans :
  forall l1 l2 l3, load_refines l1 l2 -> load_refines l2 l3 -> load_refines l1 l3.
Proof. unfold load_refines; intros l1 l2 l3 [-> | ->] [-> | ->]; auto. Qed.

Lemma refines_refl : forall o, refines o o.
Proof. now left. Qed.

Lemma refines_trans : forall o1 o2 o3, refines o1 o2 -> refines o2 o3 -> refines o1 o3.
Proof. unfold refines; intros o1 o2 o3 [-> | ->] [-> | ->]; auto. Qed.

Section Runtime.
  Context {label : Type}.

  (** ** Values (value.rs) *)

  Inductive word : Type :=
  | WInt (n : Z)
  | WFunc (l : label)
  | WClos (a : addr)
  | WCon (t : tag) (a : option addr)      (** [None]: nullary, [HeapAddress::NULL] *)
  | WBytes (a : addr).

  Definition bool_word (b : bool) : word := WCon (if b then 1 else 0) None.

  Definition prim_word (r : prim_result) : word :=
    match r with PInt n => WInt n | PBool b => bool_word b end.

  (** ** Heap objects (arena.rs, VM.md "Heap objects") *)

  Inductive obj : Type :=
  | OClosure (l : label) (env : list word)
  | OFields (ws : list word)
  | OBytes (s : list Byte.byte).

  (** Size in words, GC header included. *)
  Definition size (o : obj) : nat :=
    match o with
    | OClosure _ env => 2 + length env
    | OFields ws => 1 + length ws
    | OBytes s => 2 + (length s + 3) / 4
    end.

  Definition heap : Type := list (addr * obj).

  Fixpoint hget (H : heap) (a : addr) : option obj :=
    match H with
    | [] => None
    | (b, o) :: H => if Nat.eqb a b then Some o else hget H a
    end.

  Definition live_size (H : heap) : nat := fold_right (fun p n => size (snd p) + n) 0 H.

  (** ** Registers (registers.rs) *)

  Definition regs : Type := reg -> word.

  Definition upd (R : regs) (r : reg) (w : word) : regs :=
    fun r' => if Nat.eqb r' r then w else R r'.

  (** [R[d+i ↦ wᵢ]] *)
  Fixpoint upd_from (R : regs) (d : reg) (ws : list word) : regs :=
    match ws with
    | [] => R
    | w :: ws => upd_from (upd R d w) (S d) ws
    end.

  (** [Registers::new()]: every register [int(0)], [NULL] the sentinel
      [null], a function value at the code address [0xFFFF]. *)
  Definition clear_regs (null : label) : regs :=
    fun r => if Nat.eqb r NULL then WFunc null else WInt 0.

  (** ** Errors (error.rs), those raised after validation *)

  Inductive error : Type :=
  | HeapOverflow
  | MatchFail (t : tag) (pc : label)
  | ByteRange (v : Z) (pc : label)
  | IntOverflow (pc : label)
  | ExternFail (slot : nat) (pc : label)
  | TypeError                              (** [int_value] on a non-integer *)
  | CallOutsideCode (pc : label)           (** [Invalid "call target outside code"] *)
  | GlobalOutOfRange                       (** [Invalid "global index out of range"] *)
  | GlobalsOverflow (needed available : nat).

  Inductive outcome : Type :=
  | Halt (w : word)
  | Trap (e : error).

  (** What a host function returns. A host may allocate, read the heap and
      call back into the VM, which changes registers, so it returns all of
      them. *)
  Inductive host_result : Type :=
  | HostOk (w : word) (R : regs) (H : heap) (hp : nat)
  | HostErr (H : heap) (hp : nat).

  (** ** Garbage collection (gc.rs) *)

  (** [has_heap_addr]: the words the collector traces. *)
  Definition points (w : word) (a : addr) : Prop :=
    match w with
    | WClos b | WBytes b | WCon _ (Some b) => a = b
    | _ => False
    end.

  Definition obj_words (o : obj) : list word :=
    match o with
    | OClosure _ env => env
    | OFields ws => ws
    | OBytes _ => []
    end.

  (** Roots: the whole register file and every global. *)
  Inductive reachable (R : regs) (G : list word) (H : heap) : addr -> Prop :=
  | reach_reg r a : r < n_regs -> points (R r) a -> reachable R G H a
  | reach_global w a : In w G -> points w a -> reachable R G H a
  | reach_heap a o w b :
      reachable R G H a -> hget H a = Some o -> In w (obj_words o) -> points w b ->
      reachable R G H b.

  (** [collect]: [H'] is [H] restricted to its reachable objects. This is
      the specification gc.rs is assumed to meet (see the header). *)
  Definition collect (R : regs) (G : list word) (H H' : heap) : Prop :=
    NoDup (map fst H') /\
    forall a o, hget H' a = Some o <-> (hget H a = Some o /\ reachable R G H a).

  (** ** Allocation ([Vm::alloc], [Arena::try_alloc], [alloc_slow]) *)

  Inductive alloc_result : Type :=
  | Allocated (H : heap) (hp : nat) (a : addr)
  | OutOfMemory (H : heap) (hp : nat).

  (** [cap] is the number of heap words; [hp] the bump pointer, the words
      allocated since the last collection. *)
  Inductive alloc (cap : nat) (R : regs) (G : list word) (H : heap) (hp : nat) (o : obj)
      : alloc_result -> Prop :=
  | alloc_fast a :
      hp + size o <= cap -> hget H a = None ->
      alloc cap R G H hp o (Allocated ((a, o) :: H) (hp + size o) a)
  | alloc_after_gc H' a :
      cap < hp + size o -> collect R G H H' ->
      live_size H' + size o <= cap -> hget H' a = None ->
      alloc cap R G H hp o (Allocated ((a, o) :: H') (live_size H' + size o) a)
  | alloc_overflow H' :
      cap < hp + size o -> collect R G H H' -> cap < live_size H' + size o ->
      alloc cap R G H hp o (OutOfMemory H' (live_size H')).

  (** ** Observation *)

  (** [reify H w v]: word [w] denotes the first-order value [v] in [H]. *)
  Inductive reify (H : heap) : word -> value -> Prop :=
  | reify_int n : reify H (WInt n) (VInt n)
  | reify_bytes a s : hget H a = Some (OBytes s) -> reify H (WBytes a) (VBytes s)
  | reify_con0 t : reify H (WCon t None) (VCon t [])
  | reify_con t a ws vs :
      hget H a = Some (OFields ws) -> Forall2 (reify H) ws vs ->
      reify H (WCon t (Some a)) (VCon t vs).

  Definition kind (e : error) : trap_kind :=
    match e with
    | HeapOverflow => KHeapOverflow
    | MatchFail t _ => KMatchFail t
    | ByteRange v _ => KByteRange v
    | IntOverflow _ => KIntOverflow
    | ExternFail j _ => KExternFail j
    | TypeError => KTypeError
    | CallOutsideCode _ => KCallOutsideCode
    | GlobalOutOfRange => KGlobalOutOfRange
    | GlobalsOverflow n m => KGlobalsOverflow n m
    end.

  (** What an observer sees of outcome [o] in the final heap [H]. *)
  Inductive observe (H : heap) : outcome -> obs -> Prop :=
  | observe_value w v : reify H w v -> observe H (Halt w) (OReturn v)
  | observe_higher w : (forall v, ~ reify H w v) -> observe H (Halt w) OHigher
  | observe_trap e : observe H (Trap e) (OTrap (kind e)).
End Runtime.

Arguments word : clear implicits.
Arguments obj : clear implicits.
Arguments heap : clear implicits.
Arguments regs : clear implicits.
Arguments error : clear implicits.
Arguments outcome : clear implicits.
Arguments host_result : clear implicits.
Arguments alloc_result : clear implicits.

(** ** Hosts

    Extern functions see words, so their type depends on [label]. A host
    family gives one host per label type; a semantics uses the member for
    its own labels. Relating two semantics needs the host to treat related
    words, registers and heaps alike: [host_parametric]. *)

Section Related.
  Context {L1 L2 : Type} (rho : L1 -> L2 -> Prop).

  (** Words equal up to code addresses related by [rho]. *)
  Inductive word_rel : word L1 -> word L2 -> Prop :=
  | wr_int n : word_rel (WInt n) (WInt n)
  | wr_func l1 l2 : rho l1 l2 -> word_rel (WFunc l1) (WFunc l2)
  | wr_clos a : word_rel (WClos a) (WClos a)
  | wr_con t a : word_rel (WCon t a) (WCon t a)
  | wr_bytes a : word_rel (WBytes a) (WBytes a).

  Inductive obj_rel : obj L1 -> obj L2 -> Prop :=
  | or_closure l1 l2 env1 env2 :
      rho l1 l2 -> Forall2 word_rel env1 env2 -> obj_rel (OClosure l1 env1) (OClosure l2 env2)
  | or_fields ws1 ws2 : Forall2 word_rel ws1 ws2 -> obj_rel (OFields ws1) (OFields ws2)
  | or_bytes s : obj_rel (OBytes s) (OBytes s).

  Definition heap_rel (H1 : heap L1) (H2 : heap L2) : Prop :=
    Forall2 (fun p1 p2 => fst p1 = fst p2 /\ obj_rel (snd p1) (snd p2)) H1 H2.

  Definition regs_rel (R1 : regs L1) (R2 : regs L2) : Prop := forall r, word_rel (R1 r) (R2 r).

  Inductive host_result_rel : host_result L1 -> host_result L2 -> Prop :=
  | hr_ok w1 w2 R1 R2 H1 H2 hp :
      word_rel w1 w2 -> regs_rel R1 R2 -> heap_rel H1 H2 ->
      host_result_rel (HostOk w1 R1 H1 hp) (HostOk w2 R2 H2 hp)
  | hr_err H1 H2 hp : heap_rel H1 H2 -> host_result_rel (HostErr H1 hp) (HostErr H2 hp).
End Related.

(** [h L j w R H hp res]: extern slot [j] called on [w] returns [res]. *)
Definition host_family : Type :=
  forall L : Type, nat -> word L -> regs L -> heap L -> nat -> host_result L -> Prop.

(** A host does not look at code addresses: on related inputs, it can
    return related results. *)
Definition host_parametric (h : host_family) : Prop :=
  forall (L1 L2 : Type) (rho : L1 -> L2 -> Prop) j w1 w2 R1 R2 H1 H2 hp res1,
    word_rel rho w1 w2 -> regs_rel rho R1 R2 -> heap_rel rho H1 H2 ->
    h L1 j w1 R1 H1 hp res1 ->
    exists res2, h L2 j w2 R2 H2 hp res2 /\ host_result_rel rho res1 res2.
