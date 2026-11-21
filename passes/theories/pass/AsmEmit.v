(** [asm_emit]: lower [asm] to bytecode
    (crates/encore_compiler/src/pass/asm_emit.rs), producing the decoded
    instructions of Machine.v. Code addresses are instruction indices
    rather than byte offsets; the encoding into bytes is not modelled.

    Layout: address 0 holds the return stub [fin a1], then one stub per
    extern slot used ([EXTERN X01 A1 j; MOV A1 X01; ENCORE CONT NULL]),
    then the defines in order. The code for an expression is followed by
    the code of the function and continuation bodies it creates. (The Rust
    emitter places those bodies in a different order; only addresses
    differ.)

    Where the Rust emitter would produce code that does not mean what the
    asm means, or that the VM rejects, this one returns [Err]:
    - [no_clobber]: the argument moves before an [ENCORE] must not
      overwrite a register read later (see AsmSem.v, [encore] is a
      parallel assignment);
    - a tag used with two arities (the Rust emitter keeps the last one);
    - the limits checked by the Rust emitter and the VM's validator:
      captures, constructor fields, byte-string literals, integer
      literals, global indices, extern slots, [UNPACK] inside the register
      file, code size. *)

From Stdlib Require Import String Strings.Byte List ZArith NArith PeanoNat Bool.
Import ListNotations.
From EncorePasses Require Import Result Runtime Correctness.
From EncorePasses Require Asm Prim Machine AsmSem.
Local Open Scope list_scope.

(** ** Limits (asm_emit.rs, VM.md "Limits") *)

Definition max_captures : nat := 125.
Definition max_ctor_arity : nat := 126.
Definition max_bytes_literal : nat := 255.
Definition max_extern : nat := 32.

(** ** Argument moves *)

(** The moves [asm_emit] emits before [ENCORE f k]: [MOV A(i+1), xᵢ],
    skipped when [xᵢ] already is [A(i+1)]. *)
Fixpoint moves (i : nat) (args : list nat) : list Machine.instr :=
  match args with
  | [] => []
  | x :: rest =>
      (if Nat.eqb x (A1 + i) then [] else [Machine.Mov (A1 + i) x]) ++ moves (S i) rest
  end.

(** No move overwrites a register read after it: a later argument, the
    function or the continuation. Under this condition [moves] followed by
    [ENCORE] is the parallel assignment of AsmSem.v. *)
Fixpoint no_clobber (f k : nat) (i : nat) (args : list nat) : bool :=
  match args with
  | [] => true
  | x :: rest =>
      let t := A1 + i in
      (Nat.eqb x t
       || (negb (existsb (Nat.eqb t) rest) && negb (Nat.eqb f t) && negb (Nat.eqb k t)))
      && no_clobber f k (S i) rest
  end.

(** ** Facts about the whole module *)

Fixpoint ext_slots_val (v : Asm.val) : list nat :=
  match v with
  | Asm.Extern j => [j]
  | Asm.ContLam _ body => ext_slots_expr body
  | _ => []
  end
with ext_slots_expr (e : Asm.expr) : list nat :=
  match e with
  | Asm.Let _ v rest => ext_slots_val v ++ ext_slots_expr rest
  | Asm.Letrec _ _ body rest => ext_slots_expr body ++ ext_slots_expr rest
  | Asm.Match _ _ cases => flat_map (fun c => match c with (_, _, b) => ext_slots_expr b end) cases
  | Asm.Encore _ _ _ | Asm.Fin _ => []
  end.

(** The (tag, arity) pairs of a [match]'s cases that unpack, and of their
    bodies ([f]). *)
Section ArityCases.
  Variable f : Asm.expr -> list (nat * nat).
  Variable b : nat.
  Fixpoint arities_cases (i : nat) (cs : list (nat * nat * Asm.expr)) : list (nat * nat) :=
    match cs with
    | [] => []
    | (n, _, body) :: cs => (if Nat.eqb n 0 then [] else [(b + i, n)]) ++ f body ++ arities_cases (S i) cs
    end.
End ArityCases.

(** The (tag, arity) pairs the arity table must hold: constructors, and
    the cases that unpack. *)
Fixpoint arities_val (v : Asm.val) : list (nat * nat) :=
  match v with
  | Asm.Ctor t fs => [(t, length fs)]
  | Asm.ContLam _ body => arities_expr body
  | _ => []
  end
with arities_expr (e : Asm.expr) : list (nat * nat) :=
  match e with
  | Asm.Let _ v rest => arities_val v ++ arities_expr rest
  | Asm.Letrec _ _ body rest => arities_expr body ++ arities_expr rest
  | Asm.Match _ b cases => arities_cases arities_expr b 0 cases
  | Asm.Encore _ _ _ | Asm.Fin _ => []
  end.

Definition arities_consistent (ps : list (nat * nat)) : bool :=
  forallb (fun p => forallb (fun q => negb (Nat.eqb (fst p) (fst q)) || Nat.eqb (snd p) (snd q)) ps) ps.

Definition arity_of (ps : list (nat * nat)) (t : nat) : nat :=
  match find (fun p => Nat.eqb (fst p) t) ps with Some (_, n) => n | None => 0 end.

Fixpoint index_of (x : nat) (l : list nat) : option nat :=
  match l with
  | [] => None
  | y :: l => if Nat.eqb x y then Some 0 else option_map S (index_of x l)
  end.

(** ** Instructions for values *)

Definition prim_instr (d : nat) (op : Prim.prim) (args : list nat) : result Machine.instr :=
  match op, args with
  | Prim.PInt Prim.Byte, [x] => Ok (Machine.IntByte d x)
  | Prim.PInt o, [x; y] =>
      match AsmSem.int_binop o with
      | Some b => Ok (Machine.IntBin b d x y)
      | None => Err ("primitive " ++ Prim.prim_name op ++ ": wrong number of arguments")
      end
  | Prim.PBytes Prim.Len, [x] => Ok (Machine.BytesLen d x)
  | Prim.PBytes Prim.Get, [x; i] => Ok (Machine.BytesGet d x i)
  | Prim.PBytes Prim.Concat, [x; y] => Ok (Machine.BytesConcat d x y)
  | Prim.PBytes Prim.Slice, [x; i; n] => Ok (Machine.BytesSlice d x i n)
  | Prim.PBytes Prim.BEq, [x; y] => Ok (Machine.BytesEq d x y)
  | _, _ => Err ("primitive " ++ Prim.prim_name op ++ ": wrong number of arguments")
  end.

(** A [match]'s first instruction: [BRANCH] for two cases, else [MATCH]. *)
Definition header_instr (r b : nat) (ls : list nat) : Machine.instr :=
  match ls with
  | [l0; l1] => Machine.Branch r b l0 l1
  | _ => Machine.Match r b ls
  end.

Section Emit.
  (** Number of globals, and the address of each extern slot's stub. *)
  Variable n_globals : nat.
  Variable stub_addr : nat -> option nat.

  (** [FUNCTION] when nothing is captured, else [CLOSURE]. *)
  Definition closure_instr (d : nat) (caps : list nat) (l : nat) : result Machine.instr :=
    match caps with
    | [] => Ok (Machine.Function d l)
    | _ => if Nat.leb (length caps) max_captures then Ok (Machine.Closure d l caps)
           else Err "too many captures"
    end.

  (** Every value except a continuation, which needs its body's address. *)
  Definition val_instr (d : nat) (v : Asm.val) : result Machine.instr :=
    match v with
    | Asm.Reg r => Ok (Machine.Mov d r)
    | Asm.Capture i => Ok (Machine.Capture d i)
    | Asm.Global i =>
        if Nat.ltb i n_globals then
          Ok (if Nat.ltb i 256 then Machine.Global d i else Machine.GlobalW d i)
        else Err "global index out of range"
    | Asm.Ctor t fs =>
        if Nat.leb (length fs) max_ctor_arity then Ok (Machine.Pack d t fs)
        else Err "constructor too wide"
    | Asm.Field r i => Ok (Machine.Field d r i)
    | Asm.VInt n =>
        if (n =? 0)%Z then Ok (Machine.Int0 d)
        else if (n =? 1)%Z then Ok (Machine.Int1 d)
        else if (n =? 2)%Z then Ok (Machine.Int2 d)
        else if in_range n then Ok (Machine.Int d n)
        else Err "integer literal out of the 24-bit range"
    | Asm.Bytes s =>
        let b := list_byte_of_string s in
        if Nat.leb (length b) max_bytes_literal then Ok (Machine.Bytes d b)
        else Err "byte-string literal too long"
    | Asm.Prim op args => prim_instr d op args
    | Asm.Extern j =>
        match stub_addr j with
        | Some a => if Nat.ltb j max_extern then Ok (Machine.Function d a) else Err "extern slot out of range"
        | None => Err "extern stub missing"
        end
    | Asm.ContLam _ _ => Err "internal error: continuation handled by emit_expr"
    end.

  (** The code of a [match]'s cases from address [at_]: for case [i],
      [UNPACK] when it binds fields, then its body ([emit]). *)
  Section Cases.
    Variable emit : nat -> Asm.expr -> result (list Machine.instr).
    Variable r b : nat.
    Fixpoint emit_cases (at_ : nat) (i : nat) (cs : list (nat * nat * Asm.expr))
        : result (list (nat * list Machine.instr)) :=
      match cs with
      | [] => Ok []
      | (n, ub, body) :: cs =>
          let* code :=
            if Nat.eqb n 0 then emit at_ body
            else if Nat.leb (ub + n) n_regs then
              let* cb := emit (S at_) body in Ok (Machine.Unpack ub (b + i) r :: cb)
            else Err "unpack writes past the register file" in
          let* rest := emit_cases (at_ + length code) (S i) cs in
          Ok ((at_, code) :: rest)
      end.
  End Cases.

  (** The code for [e] placed at address [pc]: its own instructions, then
      the bodies of the functions and continuations it creates. *)
  Fixpoint emit_expr (pc : nat) (e : Asm.expr) : result (list Machine.instr) :=
    match e with
    | Asm.Let d (Asm.ContLam caps body) rest =>
        let* cr := emit_expr (S pc) rest in
        let lb := S pc + length cr in
        let* cb := emit_expr lb body in
        let* i := closure_instr d caps lb in
        Ok (i :: cr ++ cb)
    | Asm.Let d v rest =>
        let* i := val_instr d v in
        let* cr := emit_expr (S pc) rest in
        Ok (i :: cr)
    | Asm.Letrec d caps body rest =>
        let* cr := emit_expr (S pc) rest in
        let lb := S pc + length cr in
        let* cb := emit_expr lb body in
        let* i := closure_instr d caps lb in
        Ok (i :: cr ++ cb)
    | Asm.Encore f args k =>
        if no_clobber f k 0 args then Ok (moves 0 args ++ [Machine.Encore f k])
        else Err "encore: an argument move would overwrite a register read later"
    | Asm.Match r b cases =>
        let* blocks := emit_cases emit_expr r b (S pc) 0 cases in
        Ok (header_instr r b (map fst blocks) :: flat_map snd blocks)
    | Asm.Fin r => Ok [Machine.Fin r]
    end.

  (** The defines, from address [pc]: their code and entry points. *)
  Fixpoint emit_defines (pc : nat) (ds : list Asm.define)
      : result (list Machine.instr * list nat) :=
    match ds with
    | [] => Ok ([], [])
    | d :: ds =>
        let* c := emit_expr pc (Asm.def_body d) in
        let* rest := emit_defines (pc + length c) ds in
        Ok (c ++ fst rest, pc :: snd rest)
    end.
End Emit.

Definition stub (j : nat) : list Machine.instr :=
  [Machine.Extern X01 A1 j; Machine.Mov A1 X01; Machine.Encore CONT NULL].

Definition emit (m : Asm.module) : result Machine.program :=
  let slots := nodup Nat.eq_dec (flat_map (fun d => ext_slots_expr (Asm.def_body d)) m) in
  let stubs := flat_map stub slots in
  let stub_addr j := option_map (fun k => 1 + 3 * k) (index_of j slots) in
  let pairs := flat_map (fun d => arities_expr (Asm.def_body d)) m in
  if negb (arities_consistent pairs) then Err "a tag is used with two arities"
  else
    let* out := emit_defines (length m) stub_addr (1 + length stubs) m in
    let code := Machine.Fin A1 :: stubs ++ fst out in
    if Nat.leb (length code) Machine.null_label then
      Ok (Machine.Program code (arity_of pairs) (snd out))
    else Err "code too large".

(** ** Correctness

    For any host that does not look at code addresses, [emit] preserves
    the observable behaviour of every asm program, observations compared
    by [load_refines]. Proved in semantics/EmitProof.v
    ([emit_correct_holds]), with no axioms. *)
Definition emit_correct : Prop :=
  forall h : host_family, host_parametric h ->
    preserves (AsmSem.behaves (h AsmSem.alabel)) (Machine.behaves (h Machine.label)) emit.
