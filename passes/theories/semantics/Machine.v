(** Semantics of the Encore VM, transcribed from crates/encore_vm/src/vm.rs
    (with int.rs, arena.rs, gc.rs, registers.rs and value.rs).

    Abstractions, each invisible to a running program:
    - Code is a list of decoded instructions and a code address is an index
      into it. The bytecode encoding and [Program::validate] are not
      modelled: the code is assumed to have passed validation (operand
      counts, [PACK]/[UNPACK] tags in the arity table, [UNPACK] inside the
      register file, global indices and extern slots in range, static jump
      targets on instructions, no fall-through).
    - Words are tagged values, not packed 32-bit words; integers are [Z]
      kept in the 24-bit range by the instructions that build them.
    - The heap maps abstract addresses to objects. The collector keeps the
      objects reachable from the 256 registers and the globals, without
      moving them: no instruction can observe an address, so compaction is
      invisible. Sizes in words and the bump pointer [hp] are those of the
      VM, so [HeapOverflow] happens exactly when it does in the VM.

    A read the VM performs without a check (e.g. [FIELD] on a non-constructor,
    [BYTES_GET] past the end) has no rule: the state is stuck. Every error
    the VM reports at run time is a [Trap]. *)

From Stdlib Require Import List ZArith NArith PeanoNat Bool Strings.Byte.
Import ListNotations.

Definition reg := nat.
Definition label := nat.
Definition addr := nat.
Definition tag := nat.

(** ** Values (value.rs) *)

Definition SELF : reg := 0.
Definition CONT : reg := 1.
Definition A1 : reg := 2.
Definition NULL : reg := 255.
Definition n_regs : nat := 256.

Definition int_min : Z := (- 2 ^ 23)%Z.
Definition int_max : Z := (2 ^ 23 - 1)%Z.
Definition in_range (n : Z) : bool := (int_min <=? n)%Z && (n <=? int_max)%Z.

Inductive word : Type :=
| WInt (n : Z)
| WFunc (l : label)
| WClos (a : addr)
| WCon (t : tag) (a : option addr)        (** [None]: nullary, [HeapAddress::NULL] *)
| WBytes (a : addr).

(** [HeapAddress::NULL] as a code address: the [NULL] register's sentinel. *)
Definition null_label : label := Nat.pow 2 16 - 1.

(** [RETURN_CONT]: code address 0 holds the compiler's return stub. *)
Definition return_cont : word := WFunc 0.

(** [Value::from_u32(0)], the argument [load] passes to every define's
    thunk: a closure word pointing at heap address 0. *)
Definition thunk_arg : word := WClos 0.

Definition bool_word (b : bool) : word := WCon (if b then 1 else 0) None.

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

Definition heap := list (addr * obj).

Fixpoint hget (H : heap) (a : addr) : option obj :=
  match H with
  | [] => None
  | (b, o) :: H => if Nat.eqb a b then Some o else hget H a
  end.

Definition live_size (H : heap) : nat := fold_right (fun p n => size (snd p) + n) 0 H.

(** ** Registers (registers.rs) *)

Definition regs := reg -> word.

Definition upd (R : regs) (r : reg) (w : word) : regs :=
  fun r' => if Nat.eqb r' r then w else R r'.

(** [R[d+i ↦ wᵢ]] *)
Fixpoint upd_from (R : regs) (d : reg) (ws : list word) : regs :=
  match ws with
  | [] => R
  | w :: ws => upd_from (upd R d w) (S d) ws
  end.

(** [Registers::new()]: every register [int(0)], [NULL] the sentinel. *)
Definition clear_regs : regs :=
  fun r => if Nat.eqb r NULL then WFunc null_label else WInt 0.

(** ** Integer operations (int.rs) *)

Inductive binop : Type :=
| Add | Sub | Mul | Eq | Lt | Le | Div | Mod | SubSat | And | Or | Xor | Shl | Shr.

Definition checked (n : Z) : option Z := if in_range n then Some n else None.

(** [Value::int] keeps the low 24 bits and [int_value] sign-extends them. *)
Definition wrap24 (n : Z) : Z :=
  let m := (n mod 2 ^ 24)%Z in if (m <=? int_max)%Z then m else (m - 2 ^ 24)%Z.

(** [None] is [IntOverflow]. *)
Definition binop_sem (o : binop) (a b : Z) : option word :=
  match o with
  | Add => option_map WInt (checked (a + b))
  | Sub => option_map WInt (checked (a - b))
  | Mul => option_map WInt (checked (a * b))
  | Eq => Some (bool_word (a =? b)%Z)
  | Lt => Some (bool_word (a <? b)%Z)
  | Le => Some (bool_word (a <=? b)%Z)
  | Div => if (b =? 0)%Z then Some (WInt 0) else option_map WInt (checked (Z.quot a b))
  | Mod => Some (WInt (if (b =? 0)%Z then a else Z.rem a b))
  | SubSat => if (a <=? b)%Z then Some (WInt 0) else option_map WInt (checked (a - b))
  | And => Some (WInt (Z.land a b))
  | Or => Some (WInt (Z.lor a b))
  | Xor => Some (WInt (Z.lxor a b))
  | Shl =>
      if (b <? 0)%Z || (a =? 0)%Z then Some (WInt 0)
      else if (24 <=? b)%Z then None
      else option_map WInt (checked (a * 2 ^ b))
  | Shr =>
      if (b <? 0)%Z || (24 <=? b)%Z then Some (WInt 0)
      else Some (WInt (wrap24 (Z.shiftr (a mod 2 ^ 24) b)))
  end.

(** ** Instructions (opcode.rs; one constructor per opcode) *)

Inductive instr : Type :=
| Mov (d r : reg)
| Capture (d : reg) (i : nat)
| Global (d : reg) (i : nat)
| GlobalW (d : reg) (i : nat)
| Closure (d : reg) (l : label) (rs : list reg)
| Function (d : reg) (l : label)
| Pack (d : reg) (t : tag) (rs : list reg)
| Field (d r : reg) (i : nat)
| Unpack (d : reg) (t : tag) (r : reg)
| Match (r : reg) (base : tag) (ls : list label)
| Branch (r : reg) (base : tag) (l0 l1 : label)
| Encore (f k : reg)
| Int (d : reg) (n : Z)
| Int0 (d : reg) | Int1 (d : reg) | Int2 (d : reg)
| IntBin (o : binop) (d a b : reg)
| IntByte (d r : reg)
| Bytes (d : reg) (s : list Byte.byte)
| BytesLen (d r : reg)
| BytesGet (d r i : reg)
| BytesConcat (d a b : reg)
| BytesSlice (d r i n : reg)
| BytesEq (d a b : reg)
| Extern (d r : reg) (j : nat)
| Fin (r : reg).

(** ** Errors (error.rs), those raised after validation *)

Inductive error : Type :=
| HeapOverflow
| MatchFail (t : tag) (pc : label)
| ByteRange (v : Z) (pc : label)
| IntOverflow (pc : label)
| ExternFail (slot : nat) (pc : label)
| TypeError                                (** [int_value] on a non-integer *)
| CallOutsideCode (pc : label)             (** [Invalid "call target outside code"] *)
| GlobalOutOfRange                         (** [Invalid "global index out of range"] *)
| GlobalsOverflow (needed available : nat).

Inductive outcome : Type :=
| Halt (w : word)
| Trap (e : error).

(** ** Machine state *)

Record state : Type := St {
  pc : label;
  R : regs;
  H : heap;
  hp : nat;                 (** bump pointer: words allocated since the last collection *)
  G : list word             (** globals *)
}.

(** A step either continues or stops; a stopped run keeps its heap. *)
Inductive result : Type :=
| Run (s : state)
| Stop (o : outcome) (H : heap) (hp : nat).

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

(** [collect]: [H'] is [H] restricted to its reachable objects. *)
Definition collect (R : regs) (G : list word) (H H' : heap) : Prop :=
  NoDup (map fst H') /\
  forall a o, hget H' a = Some o <-> (hget H a = Some o /\ reachable R G H a).

Section Machine.
  (** The validated code. *)
  Variable code : list instr.
  (** The program's arity table. *)
  Variable arity : tag -> nat.
  (** Usable words of the memory buffer: [min(len, 0xFFFF)]. *)
  Variable mem_words : nat.
  (** Entry point of each define, in order. *)
  Variable entries : list label.
  (** [extern_fns[j]] applied to its argument; an unregistered slot fails. *)
  Variable host : nat -> word -> regs -> heap -> nat -> host_result -> Prop.

  (** [load] carves one word per global off the top of the buffer. *)
  Definition cap : nat := mem_words - length entries.

  (** ** Allocation ([Vm::alloc], [Arena::try_alloc], [alloc_slow]) *)

  Inductive alloc_result : Type :=
  | Allocated (H : heap) (hp : nat) (a : addr)
  | OutOfMemory (H : heap) (hp : nat).

  Inductive alloc (R : regs) (G : list word) (H : heap) (hp : nat) (o : obj)
      : alloc_result -> Prop :=
  | alloc_fast a :
      hp + size o <= cap -> hget H a = None ->
      alloc R G H hp o (Allocated ((a, o) :: H) (hp + size o) a)
  | alloc_after_gc H' a :
      cap < hp + size o -> collect R G H H' ->
      live_size H' + size o <= cap -> hget H' a = None ->
      alloc R G H hp o (Allocated ((a, o) :: H') (live_size H' + size o) a)
  | alloc_overflow H' :
      cap < hp + size o -> collect R G H H' -> cap < live_size H' + size o ->
      alloc R G H hp o (OutOfMemory H' (live_size H')).

  Definition fetch (s : state) : option instr := nth_error code (pc s).

  Definition next (s : state) (R' : regs) : result :=
    Run (St (S (pc s)) R' (H s) (hp s) (G s)).

  Definition trap (s : state) (e : error) : result := Stop (Trap e) (H s) (hp s).

  (** Allocate [o] and put [mk a] in [d], or trap with [HeapOverflow]. *)
  Inductive allocate (s : state) (o : obj) (d : reg) (mk : addr -> word) : result -> Prop :=
  | allocate_ok H' hp' a :
      alloc (R s) (G s) (H s) (hp s) o (Allocated H' hp' a) ->
      allocate s o d mk (Run (St (S (pc s)) (upd (R s) d (mk a)) H' hp' (G s)))
  | allocate_oom H' hp' :
      alloc (R s) (G s) (H s) (hp s) o (OutOfMemory H' hp') ->
      allocate s o d mk (Stop (Trap HeapOverflow) H' hp').

  Definition as_int (w : word) : option Z :=
    match w with WInt n => Some n | _ => None end.

  Definition bytes_of (s : state) (w : word) : option (list Byte.byte) :=
    match w with
    | WBytes a => match hget (H s) a with Some (OBytes b) => Some b | _ => None end
    | _ => None
    end.

  (** [resolve_code_ptr] before its range check. *)
  Definition code_of (Hp : heap) (w : word) : option label :=
    match w with
    | WFunc l => Some l
    | WClos a => match hget Hp a with Some (OClosure l _) => Some l | _ => None end
    | _ => None
    end.

  (** ** One instruction ([Vm::run]) *)

  Inductive step (s : state) : result -> Prop :=
  | s_mov d r :
      fetch s = Some (Mov d r) -> step s (next s (upd (R s) d (R s r)))
  | s_capture d i a l env w :
      fetch s = Some (Capture d i) ->
      R s SELF = WClos a -> hget (H s) a = Some (OClosure l env) -> nth_error env i = Some w ->
      step s (next s (upd (R s) d w))
  | s_global d i w :
      fetch s = Some (Global d i) -> nth_error (G s) i = Some w ->
      step s (next s (upd (R s) d w))
  | s_global_w d i w :
      fetch s = Some (GlobalW d i) -> nth_error (G s) i = Some w ->
      step s (next s (upd (R s) d w))
  | s_closure d l rs r :
      fetch s = Some (Closure d l rs) ->
      allocate s (OClosure l (map (R s) rs)) d WClos r -> step s r
  | s_function d l :
      fetch s = Some (Function d l) -> step s (next s (upd (R s) d (WFunc l)))
  | s_pack_nullary d t :
      fetch s = Some (Pack d t []) -> arity t = 0 ->
      step s (next s (upd (R s) d (WCon t None)))
  | s_pack d t rs r :
      fetch s = Some (Pack d t rs) -> arity t > 0 -> length rs = arity t ->
      allocate s (OFields (map (R s) rs)) d (fun a => WCon t (Some a)) r -> step s r
  | s_field d r i t a ws w :
      fetch s = Some (Field d r i) ->
      R s r = WCon t (Some a) -> hget (H s) a = Some (OFields ws) -> nth_error ws i = Some w ->
      step s (next s (upd (R s) d w))
  | s_unpack_nullary d t r :
      (* the copy loop runs zero times and reads nothing *)
      fetch s = Some (Unpack d t r) -> arity t = 0 -> step s (next s (R s))
  | s_unpack d t r t' a ws :
      fetch s = Some (Unpack d t r) -> arity t > 0 ->
      R s r = WCon t' (Some a) -> hget (H s) a = Some (OFields ws) -> arity t <= length ws ->
      step s (next s (upd_from (R s) d (firstn (arity t) ws)))
  | s_match r b ls t oa l :
      (* [tag.wrapping_sub(base)] on [u8] *)
      fetch s = Some (Match r b ls) -> R s r = WCon t oa ->
      nth_error ls ((t + 256 - b) mod 256) = Some l ->
      step s (Run (St l (R s) (H s) (hp s) (G s)))
  | s_match_fail r b ls t oa :
      fetch s = Some (Match r b ls) -> R s r = WCon t oa ->
      length ls <= (t + 256 - b) mod 256 ->
      step s (trap s (MatchFail t (pc s)))
  | s_branch r b l0 l1 t oa :
      fetch s = Some (Branch r b l0 l1) -> R s r = WCon t oa ->
      step s (Run (St (if Nat.eqb t b then l0 else l1) (R s) (H s) (hp s) (G s)))
  | s_encore f k l i :
      fetch s = Some (Encore f k) -> code_of (H s) (R s f) = Some l -> nth_error code l = Some i ->
      step s (Run (St l (upd (upd (R s) SELF (R s f)) CONT (R s k)) (H s) (hp s) (G s)))
  | s_encore_outside f k l :
      fetch s = Some (Encore f k) -> code_of (H s) (R s f) = Some l -> nth_error code l = None ->
      step s (trap s (CallOutsideCode (pc s)))
  | s_int d n :
      fetch s = Some (Int d n) -> step s (next s (upd (R s) d (WInt n)))
  | s_int0 d : fetch s = Some (Int0 d) -> step s (next s (upd (R s) d (WInt 0)))
  | s_int1 d : fetch s = Some (Int1 d) -> step s (next s (upd (R s) d (WInt 1)))
  | s_int2 d : fetch s = Some (Int2 d) -> step s (next s (upd (R s) d (WInt 2)))
  | s_intbin o d a b x y w :
      fetch s = Some (IntBin o d a b) -> as_int (R s a) = Some x -> as_int (R s b) = Some y ->
      binop_sem o x y = Some w -> step s (next s (upd (R s) d w))
  | s_intbin_overflow o d a b x y :
      fetch s = Some (IntBin o d a b) -> as_int (R s a) = Some x -> as_int (R s b) = Some y ->
      binop_sem o x y = None -> step s (trap s (IntOverflow (pc s)))
  | s_intbin_type o d a b :
      fetch s = Some (IntBin o d a b) -> as_int (R s a) = None \/ as_int (R s b) = None ->
      step s (trap s TypeError)
  | s_int_byte d r n c res :
      fetch s = Some (IntByte d r) -> R s r = WInt n -> Byte.of_N (Z.to_N n) = Some c ->
      (0 <= n)%Z -> allocate s (OBytes [c]) d WBytes res -> step s res
  | s_int_byte_range d r n :
      fetch s = Some (IntByte d r) -> R s r = WInt n -> (n < 0 \/ 255 < n)%Z ->
      step s (trap s (ByteRange n (pc s)))
  | s_int_byte_type d r :
      fetch s = Some (IntByte d r) -> as_int (R s r) = None -> step s (trap s TypeError)
  | s_bytes d b res :
      fetch s = Some (Bytes d b) -> allocate s (OBytes b) d WBytes res -> step s res
  | s_bytes_len d r b :
      fetch s = Some (BytesLen d r) -> bytes_of s (R s r) = Some b ->
      step s (next s (upd (R s) d (WInt (Z.of_nat (length b)))))
  | s_bytes_get d r i n b c :
      fetch s = Some (BytesGet d r i) -> R s i = WInt n -> bytes_of s (R s r) = Some b ->
      (0 <= n)%Z -> nth_error b (Z.to_nat n) = Some c ->
      step s (next s (upd (R s) d (WInt (Z.of_N (Byte.to_N c)))))
  | s_bytes_get_type d r i :
      fetch s = Some (BytesGet d r i) -> as_int (R s i) = None -> step s (trap s TypeError)
  | s_bytes_concat d a b x y res :
      fetch s = Some (BytesConcat d a b) ->
      bytes_of s (R s a) = Some x -> bytes_of s (R s b) = Some y ->
      allocate s (OBytes (x ++ y)) d WBytes res -> step s res
  | s_bytes_slice d r i n st len b res :
      fetch s = Some (BytesSlice d r i n) -> R s i = WInt st -> R s n = WInt len ->
      (0 <= st)%Z -> (0 <= len)%Z -> bytes_of s (R s r) = Some b ->
      Z.to_nat st + Z.to_nat len <= length b ->
      allocate s (OBytes (firstn (Z.to_nat len) (skipn (Z.to_nat st) b))) d WBytes res ->
      step s res
  | s_bytes_slice_oom d r i n st len H' hp' :
      (* the VM allocates before it reads the source *)
      fetch s = Some (BytesSlice d r i n) -> R s i = WInt st -> R s n = WInt len -> (0 <= len)%Z ->
      alloc (R s) (G s) (H s) (hp s) (OBytes (repeat Byte.x00 (Z.to_nat len))) (OutOfMemory H' hp') ->
      step s (Stop (Trap HeapOverflow) H' hp')
  | s_bytes_slice_type d r i n :
      fetch s = Some (BytesSlice d r i n) -> as_int (R s i) = None \/ as_int (R s n) = None ->
      step s (trap s TypeError)
  | s_bytes_eq d a b x y :
      fetch s = Some (BytesEq d a b) ->
      bytes_of s (R s a) = Some x -> bytes_of s (R s b) = Some y ->
      step s (next s (upd (R s) d (bool_word (if list_eq_dec Byte.byte_eq_dec x y then true else false))))
  | s_extern d r j w R' H' hp' :
      fetch s = Some (Extern d r j) -> host j (R s r) (R s) (H s) (hp s) (HostOk w R' H' hp') ->
      step s (Run (St (S (pc s)) (upd R' d w) H' hp' (G s)))
  | s_extern_fail d r j H' hp' :
      fetch s = Some (Extern d r j) -> host j (R s r) (R s) (H s) (hp s) (HostErr H' hp') ->
      step s (Stop (Trap (ExternFail j (pc s))) H' hp')
  | s_fin r :
      fetch s = Some (Fin r) -> step s (Stop (Halt (R s r)) (H s) (hp s)).

  (** ** Running to completion ([run_to_completion]) *)

  (** No instruction writes a global, so a run keeps [G]. *)
  Inductive exec : state -> outcome -> heap -> nat -> Prop :=
  | exec_stop s o H' hp' : step s (Stop o H' hp') -> exec s o H' hp'
  | exec_run s s' o H' hp' : step s (Run s') -> exec s' o H' hp' -> exec s o H' hp'.

  (** Between calls the registers are cleared ([Registers::new()]), so the
      VM at rest is its heap, bump pointer and globals. *)
  Record vm : Type := Vm { vheap : heap; vhp : nat; vglobals : list word }.

  Definition init : vm := Vm [] 0 [].

  (** [call_raw]: SELF, CONT and A1.. on cleared registers. *)
  Definition entry_regs (f : word) (args : list word) : regs :=
    upd_from (upd (upd clear_regs SELF f) CONT return_cont) A1 args.

  Inductive call_value (m : vm) (f : word) (args : list word) : outcome -> vm -> Prop :=
  | call_ok l i o H' hp' :
      code_of (vheap m) f = Some l -> nth_error code l = Some i ->
      exec (St l (entry_regs f args) (vheap m) (vhp m) (vglobals m)) o H' hp' ->
      call_value m f args o (Vm H' hp' (vglobals m))
  | call_outside l p :
      (* [p] is the stale [pc] the VM reports *)
      code_of (vheap m) f = Some l -> nth_error code l = None ->
      call_value m f args (Trap (CallOutsideCode p)) m.

  (** [call_global_raw] *)
  Inductive call_global (m : vm) (idx : nat) (args : list word) : outcome -> vm -> Prop :=
  | call_global_ok f o m' :
      nth_error (vglobals m) idx = Some f -> call_value m f args o m' ->
      call_global m idx args o m'
  | call_global_range :
      nth_error (vglobals m) idx = None -> call_global m idx args (Trap GlobalOutOfRange) m.

  (** [call_address]: a define's thunk, with no range check (validated). *)
  Inductive call_address (m : vm) (l : label) : outcome -> vm -> Prop :=
  | call_address_run o H' hp' :
      exec (St l (entry_regs (WFunc l) [thunk_arg]) (vheap m) (vhp m) (vglobals m)) o H' hp' ->
      call_address m l o (Vm H' hp' (vglobals m)).

  Definition set_global (m : vm) (i : nat) (w : word) : vm :=
    Vm (vheap m) (vhp m) (firstn i (vglobals m) ++ w :: skipn (S i) (vglobals m)).

  (** Run each define's thunk in order and store its result; the first
      error aborts the load. *)
  Inductive load_defines : nat -> list label -> vm -> option error -> vm -> Prop :=
  | load_done i m : load_defines i [] m None m
  | load_next i l ls m w m' r m'' :
      call_address m l (Halt w) m' -> load_defines (S i) ls (set_global m' i w) r m'' ->
      load_defines i (l :: ls) m r m''
  | load_fail i l ls m e m' :
      call_address m l (Trap e) m' -> load_defines i (l :: ls) m (Some e) m'.

  (** [Vm::load] on a fresh VM ([init]): globals start as [int(0)]. *)
  Inductive load : option error -> vm -> Prop :=
  | load_overflow :
      mem_words < length entries ->
      load (Some (GlobalsOverflow (length entries) mem_words)) init
  | load_run r m :
      length entries <= mem_words ->
      load_defines 0 entries (Vm [] 0 (repeat (WInt 0) (length entries))) r m ->
      load r m.
End Machine.
