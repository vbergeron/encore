(** The bytecode machine of the Encore paper ("From Rocq to Metal",
    paper/semantics.tex, Fig. "Bytecode machine"), transcribed as written.

    Words abstract the 32-bit packed encoding, the heap is a partial map
    from addresses to objects, and the code [c] is left abstract: [c pc] is
    the instruction at [pc] and [next pc] is [pc⁺]. A state with no
    applicable rule is stuck. *)

From Stdlib Require Import List ZArith PeanoNat.
Import ListNotations.

Definition reg := nat.
Definition label := nat.
Definition addr := nat.
Definition tag := nat.

(** Register indices (VM.md). *)
Definition SELF : reg := 0.
Definition CONT : reg := 1.

(** [w ::= int(n) | func(ℓ) | clos(a) | con(t, a) | con(t, •) | bytes(a)];
    [con t None] is [con(t, •)]. *)
Inductive word : Type :=
| WInt (n : Z)
| WFunc (l : label)
| WClos (a : addr)
| WCon (t : tag) (a : option addr)
| WBytes (a : addr).

(** [H(a) ::= ⟨ℓ; w₀ … wₘ₋₁⟩ | ⟨w₀ … wₘ₋₁⟩ | s | w] *)
Inductive obj : Type :=
| OClosure (l : label) (env : list word)
| OFields (ws : list word)
| OBytes (s : list Byte.byte)
| OCell (w : word).                       (** global cell *)

Definition heap := addr -> option obj.
Definition regs := reg -> word.

Definition upd {A} (f : nat -> A) (n : nat) (x : A) : nat -> A :=
  fun m => if Nat.eqb m n then x else f m.

Definition halloc (H : heap) (a : addr) (o : obj) : heap := upd H a (Some o).

(** [R[d+i ↦ wᵢ]ᵢ] *)
Fixpoint upd_from (R : regs) (d : reg) (ws : list word) : regs :=
  match ws with
  | [] => R
  | w :: ws => upd_from (upd R d w) (S d) ws
  end.

(** [⟨pc, R, H⟩] *)
Record state : Type := St { pc : label; rf : regs; hp : heap }.

(** [op] stands for the literal, integer and byte-string opcodes. *)
Inductive instr (op : Type) : Type :=
| Mov (d r : reg)
| Capture (d : reg) (i : nat)
| Global (d : reg) (i : nat)
| GlobalW (d : reg) (i : nat)             (** [global] with a 16-bit index *)
| Function (d : reg) (l : label)
| Closure (d : reg) (l : label) (rs : list reg)
| Pack (d : reg) (t : tag) (rs : list reg)
| Field (d r : reg) (i : nat)
| Unpack (d : reg) (t : tag) (r : reg)
| Match (r : reg) (b : tag) (ls : list label)
| Branch (r : reg) (b : tag) (l0 l1 : label)
| Encore (f k : reg)
| Extern (d r : reg) (j : nat)
| Op (o : op) (d : reg) (rs : list reg)
| Fin (r : reg).

Arguments Mov {op}. Arguments Capture {op}. Arguments Global {op}.
Arguments GlobalW {op}. Arguments Function {op}. Arguments Closure {op}.
Arguments Pack {op}. Arguments Field {op}. Arguments Unpack {op}.
Arguments Match {op}. Arguments Branch {op}. Arguments Encore {op}.
Arguments Extern {op}. Arguments Op {op}. Arguments Fin {op}.

Section Machine.
  Context {op : Type}.
  (** [c(pc)] and [pc⁺] *)
  Variable c : label -> option (instr op).
  Variable next : label -> label.
  (** [arity(t)], the constructor arity table *)
  Variable arity : tag -> nat.
  (** [gᵢ], the reserved heap cell of global [i] *)
  Variable g : nat -> addr.
  (** [h_j], the host function registered in extern slot [j] *)
  Variable host : nat -> word -> heap -> option (word * heap).
  (** [δ_op], partial: undefined exactly where the VM traps *)
  Variable delta : op -> list word -> heap -> option (word * heap).

  (** [code_H] *)
  Definition code (H : heap) (w : word) : option label :=
    match w with
    | WFunc l => Some l
    | WClos a => match H a with Some (OClosure l _) => Some l | _ => None end
    | _ => None
    end.

  Inductive step : state -> state -> Prop :=
  | step_mov pc R H d r :
      c pc = Some (Mov d r) ->
      step (St pc R H) (St (next pc) (upd R d (R r)) H)
  | step_capture pc R H d i a l ws w :
      c pc = Some (Capture d i) ->
      R SELF = WClos a -> H a = Some (OClosure l ws) -> nth_error ws i = Some w ->
      step (St pc R H) (St (next pc) (upd R d w) H)
  | step_global pc R H d i w :
      c pc = Some (Global d i) -> H (g i) = Some (OCell w) ->
      step (St pc R H) (St (next pc) (upd R d w) H)
  | step_global_w pc R H d i w :
      c pc = Some (GlobalW d i) -> H (g i) = Some (OCell w) ->
      step (St pc R H) (St (next pc) (upd R d w) H)
  | step_function pc R H d l :
      c pc = Some (Function d l) ->
      step (St pc R H) (St (next pc) (upd R d (WFunc l)) H)
  | step_closure pc R H d l rs a :
      c pc = Some (Closure d l rs) -> H a = None ->
      step (St pc R H)
           (St (next pc) (upd R d (WClos a)) (halloc H a (OClosure l (map R rs))))
  | step_pack pc R H d t rs a :
      c pc = Some (Pack d t rs) -> arity t > 0 -> H a = None ->
      step (St pc R H)
           (St (next pc) (upd R d (WCon t (Some a))) (halloc H a (OFields (map R rs))))
  | step_pack_nullary pc R H d t :
      c pc = Some (Pack d t []) -> arity t = 0 ->
      step (St pc R H) (St (next pc) (upd R d (WCon t None)) H)
  | step_field pc R H d r i t a ws w :
      c pc = Some (Field d r i) ->
      R r = WCon t (Some a) -> H a = Some (OFields ws) -> nth_error ws i = Some w ->
      step (St pc R H) (St (next pc) (upd R d w) H)
  | step_unpack pc R H d t r t' a ws :
      c pc = Some (Unpack d t r) ->
      R r = WCon t' (Some a) -> H a = Some (OFields ws) -> arity t <= length ws ->
      step (St pc R H) (St (next pc) (upd_from R d (firstn (arity t) ws)) H)
  | step_match pc R H r b ls t oa l :
      c pc = Some (Match r b ls) ->
      R r = WCon t oa -> b <= t -> nth_error ls (t - b) = Some l ->
      step (St pc R H) (St l R H)
  | step_branch_eq pc R H r b l0 l1 oa :
      c pc = Some (Branch r b l0 l1) -> R r = WCon b oa ->
      step (St pc R H) (St l0 R H)
  | step_branch_ne pc R H r b l0 l1 t oa :
      c pc = Some (Branch r b l0 l1) -> R r = WCon t oa -> t <> b ->
      step (St pc R H) (St l1 R H)
  | step_encore pc R H f k l i :
      c pc = Some (Encore f k) -> code H (R f) = Some l -> c l = Some i ->
      step (St pc R H) (St l (upd (upd R SELF (R f)) CONT (R k)) H)
  | step_extern pc R H d r j w H' :
      c pc = Some (Extern d r j) -> host j (R r) H = Some (w, H') ->
      step (St pc R H) (St (next pc) (upd R d w) H')
  | step_op pc R H o d rs w H' :
      c pc = Some (Op o d rs) -> delta o (map R rs) H = Some (w, H') ->
      step (St pc R H) (St (next pc) (upd R d w) H').

  (** [fin r]: halt with [R(r)]. *)
  Inductive halts : state -> word -> Prop :=
  | halts_fin pc R H r :
      c pc = Some (Fin r) -> halts (St pc R H) (R r).
End Machine.
