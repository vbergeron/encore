(** [asm]: registers, captures and globals, no names
    (crates/encore_compiler/src/ir/asm.rs). Stages [asm-resolve] and
    [asm-peephole]. *)

From Stdlib Require Import String Ascii List ZArith.
From EncorePasses Require Import Result Sexp Prim.
Import ListNotations.
Open Scope string_scope.

(** A register index, [0..255]. *)
Definition reg := nat.

Definition SELF : reg := 0.
Definition CONT : reg := 1.
Definition A1 : reg := 2.
Definition A8 : reg := 9.
Definition NULL : reg := 255.

Inductive val : Type :=
| Reg (r : reg)
| Capture (idx : nat)
| Global (idx : nat)
| ContLam (captures : list reg) (body : expr)
| Ctor (tag : nat) (fields : list reg)
| Field (r : reg) (idx : nat)
| VInt (z : Z)
| Bytes (b : string)
| Prim (op : prim) (args : list reg)
| Extern (slot : nat)

with expr : Type :=
| Let (r : reg) (v : val) (body : expr)
| Letrec (r : reg) (captures : list reg) (fun_body rest : expr)
                                     (** closure capturing [captures] *)
| Encore (f : reg) (args : list reg) (k : reg)
| Match (scrut : reg) (base : nat) (cases : list (nat * reg * expr))
                                     (** (arity, unpack base, body) *)
| Fin (r : reg).

Record define : Type := { def_global : nat; def_body : expr }.
Definition module : Type := list define.

(** ** Register names: [self], [cont], [a1]..[a8], [null], [r0]..[r255] *)

Definition reg_name (r : reg) : string :=
  if Nat.eqb r SELF then "self"
  else if Nat.eqb r CONT then "cont"
  else if Nat.eqb r NULL then "null"
  else if andb (Nat.leb A1 r) (Nat.leb r A8) then "a" ++ nat_to_string (r - A1 + 1)
  else "r" ++ nat_to_string r.

(** Digit values are capped at [digits_cap], out of range for every caller. *)
Definition digits_cap : nat := 1000.

Fixpoint parse_digits (s : string) (acc : nat) : option nat :=
  match s with
  | EmptyString => Some acc
  | String c rest =>
      let d := nat_of_ascii c in
      if andb (Nat.leb 48 d) (Nat.leb d 57)
      then parse_digits rest (Nat.min digits_cap (acc * 10 + (d - 48)))
      else None
  end.

Definition parse_num (prefix : ascii) (s : string) : option nat :=
  match s with
  | String c (String _ _ as digits) =>
      if Ascii.eqb c prefix then parse_digits digits 0 else None
  | _ => None
  end.

Definition parse_reg_name (s : string) : option reg :=
  if String.eqb s "self" then Some SELF
  else if String.eqb s "cont" then Some CONT
  else if String.eqb s "null" then Some NULL
  else match parse_num "a"%char s, parse_num "r"%char s with
       | Some n, _ => if andb (Nat.leb 1 n) (Nat.leb n 8) then Some (A1 + (n - 1)) else None
       | _, Some n => if Nat.leb n 255 then Some n else None
       | None, None => None
       end.

Definition encode_reg (r : reg) : sexp := Sym (reg_name r).

Definition decode_reg (s : sexp) : result reg :=
  let* x := as_sym s in
  match parse_reg_name x with
  | Some r => Ok r
  | None => Err ("expected a register (self, cont, a1..a8, null, r0..r255), found `" ++ x ++ "`")
  end.

Definition decode_regs (s : sexp) : result (list reg) :=
  let* l := as_list s in mapM decode_reg l.

(** ** Text form *)

Fixpoint encode_val (v : val) : sexp :=
  match v with
  | Reg r => encode_reg r
  | VInt z => Int z
  | Capture i => form "capture" [nat_sexp i]
  | Global g => form "global" [nat_sexp g]
  | ContLam cs b => form "cont-lam" [List (map encode_reg cs); encode_expr b]
  | Ctor t fs => form "ctor" (nat_sexp t :: map encode_reg fs)
  | Field r i => form "field" [encode_reg r; nat_sexp i]
  | Bytes b => form "bytes" [Str b]
  | Prim op args => form "prim" (encode_prim op :: map encode_reg args)
  | Extern n => form "extern" [nat_sexp n]
  end

with encode_expr (e : expr) : sexp :=
  match e with
  | Let r v b => form "let" [encode_reg r; encode_val v; encode_expr b]
  | Letrec r cs b rest =>
      form "letrec" [encode_reg r; List (map encode_reg cs); encode_expr b; encode_expr rest]
  | Encore f args k => form "encore" [encode_reg f; List (map encode_reg args); encode_reg k]
  | Match s b cs =>
      form "match" (encode_reg s :: nat_sexp b
        :: map (fun c => match c with
                         | (n, ub, body) => form "case" [nat_sexp n; encode_reg ub; encode_expr body]
                         end) cs)
  | Fin r => form "fin" [encode_reg r]
  end.

Fixpoint decode_val (s : sexp) : result val :=
  match s with
  | Sym _ => fmap Reg (decode_reg s)
  | Int _ => fmap VInt (as_i32 s)
  | List (Sym h :: a) =>
      if String.eqb h "capture" then
        match a with
        | [i] => fmap Capture (as_u8 "capture index" i)
        | _ => arity_err h 1 a
        end
      else if String.eqb h "global" then
        match a with
        | [i] => fmap Global (as_u16 "global index" i)
        | _ => arity_err h 1 a
        end
      else if String.eqb h "cont-lam" then
        match a with
        | [cs; b] => let* cs := decode_regs cs in let* b := decode_expr b in Ok (ContLam cs b)
        | _ => arity_err h 2 a
        end
      else if String.eqb h "ctor" then
        match a with
        | t :: fs => let* t := as_u8 "tag" t in let* fs := mapM decode_reg fs in Ok (Ctor t fs)
        | _ => arity_at_least_err h 1 a
        end
      else if String.eqb h "field" then
        match a with
        | [r; i] => let* r := decode_reg r in let* i := as_u8 "field index" i in Ok (Field r i)
        | _ => arity_err h 2 a
        end
      else if String.eqb h "bytes" then
        match a with
        | [b] => fmap Bytes (as_str b)
        | _ => arity_err h 1 a
        end
      else if String.eqb h "prim" then
        match a with
        | op :: args =>
            let* op := decode_prim op in let* args := mapM decode_reg args in Ok (Prim op args)
        | _ => arity_at_least_err h 1 a
        end
      else if String.eqb h "extern" then
        match a with
        | [n] => fmap Extern (as_u16 "extern slot" n)
        | _ => arity_err h 1 a
        end
      else Err ("unknown asm value form `" ++ h ++ "`")
  | _ => Err ("expected an asm value, found " ++ describe s)
  end

with decode_expr (s : sexp) : result expr :=
  match s with
  | List (Sym h :: a) =>
      if String.eqb h "let" then
        match a with
        | [r; v; b] =>
            let* r := decode_reg r in let* v := decode_val v in
            let* b := decode_expr b in Ok (Let r v b)
        | _ => arity_err h 3 a
        end
      else if String.eqb h "letrec" then
        match a with
        | [r; cs; b; rest] =>
            let* r := decode_reg r in let* cs := decode_regs cs in
            let* b := decode_expr b in let* rest := decode_expr rest in Ok (Letrec r cs b rest)
        | _ => arity_err h 4 a
        end
      else if String.eqb h "encore" then
        match a with
        | [f; args; k] =>
            let* f := decode_reg f in let* args := decode_regs args in
            let* k := decode_reg k in Ok (Encore f args k)
        | _ => arity_err h 3 a
        end
      else if String.eqb h "match" then
        match a with
        | sc :: b :: cs =>
            let* sc := decode_reg sc in
            let* b := as_u8 "tag" b in
            let* cs := mapM (fun c =>
              match c with
              | List (Sym ch :: ca) =>
                  if String.eqb ch "case" then
                    match ca with
                    | [n; ub; body] =>
                        let* n := as_u8 "arity" n in let* ub := decode_reg ub in
                        let* body := decode_expr body in Ok (n, ub, body)
                    | _ => arity_err "case" 3 ca
                    end
                  else Err ("expected `(case ...)`, found " ++ describe c)
              | _ => Err ("expected `(case ...)`, found " ++ describe c)
              end) cs in
            Ok (Match sc b cs)
        | _ => arity_at_least_err h 2 a
        end
      else if String.eqb h "fin" then
        match a with
        | [r] => fmap Fin (decode_reg r)
        | _ => arity_err h 1 a
        end
      else Err ("unknown asm expression form `" ++ h ++ "`")
  | _ => Err ("expected an asm expression, found " ++ describe s)
  end.

Definition header : string := "encore ir: asm".

Definition encode (m : module) : list sexp :=
  map (fun d => form "define" [nat_sexp (def_global d); encode_expr (def_body d)]) m.

Definition decode (forms : list sexp) : result module :=
  decode_defines (fun g b =>
    let* g := as_u16 "global index" g in let* b := decode_expr b in
    Ok {| def_global := g; def_body := b |}) forms.
