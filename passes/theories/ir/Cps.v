(** [cps]: continuation-passing style, named (crates/encore_compiler/src/ir/cps.rs).
    Stages [cps-transform] and [cps-optimize]. *)

From Stdlib Require Import String List ZArith.
From EncorePasses Require Import Result Sexp Prim.
Import ListNotations.
Open Scope string_scope.

Definition name := string.

(** Values and expressions are mutually recursive: a [Cont] value holds an
    expression. *)
Inductive val : Type :=
| Var (x : name)
| Cont (params : list name) (body : expr)
| NullCont
| Ctor (tag : nat) (fields : list name)
| Field (x : name) (idx : nat)
| VInt (z : Z)
| Bytes (b : string)
| Prim (op : prim) (args : list name)
| Extern (slot : nat)

with expr : Type :=
| Let (x : name) (v : val) (body : expr)
| Letrec (f : name) (args : list name) (k : name) (fun_body rest : expr)
                                     (** f(args; k) = fun_body in rest *)
| Encore (f : name) (args : list name) (k : name)          (** tail call *)
| Match (scrut : name) (base : nat) (cases : list (list name * expr))
| Fin (x : name).

Record define : Type := { def_name : name; def_body : expr }.
Definition module : Type := list define.

(** ** Text form *)

Fixpoint encode_val (v : val) : sexp :=
  match v with
  | Var x => Sym x
  | VInt z => Int z
  | Cont ps b => form "cont" [List (syms ps); encode_expr b]
  | NullCont => form "null-cont" []
  | Ctor t fs => form "ctor" (nat_sexp t :: syms fs)
  | Field x i => form "field" [Sym x; nat_sexp i]
  | Bytes b => form "bytes" [Str b]
  | Prim op args => form "prim" (encode_prim op :: syms args)
  | Extern n => form "extern" [nat_sexp n]
  end

with encode_expr (e : expr) : sexp :=
  match e with
  | Let x v b => form "let" [Sym x; encode_val v; encode_expr b]
  | Letrec f args k b r =>
      form "letrec" [Sym f; List (syms args); Sym k; encode_expr b; encode_expr r]
  | Encore f args k => form "encore" [Sym f; List (syms args); Sym k]
  | Match s b cs =>
      form "match" (Sym s :: nat_sexp b
        :: map (fun c => form "case" [List (syms (fst c)); encode_expr (snd c)]) cs)
  | Fin x => form "fin" [Sym x]
  end.

(** Decoding recurses structurally on the [sexp]. *)
Fixpoint decode_val (s : sexp) : result val :=
  match s with
  | Sym x => Ok (Var x)
  | Int _ => fmap VInt (as_i32 s)
  | List (Sym h :: a) =>
      if String.eqb h "cont" then
        match a with
        | [ps; b] => let* ps := sym_list ps in let* b := decode_expr b in Ok (Cont ps b)
        | _ => arity_err h 2 a
        end
      else if String.eqb h "null-cont" then
        match a with
        | [] => Ok NullCont
        | _ => arity_err h 0 a
        end
      else if String.eqb h "ctor" then
        match a with
        | t :: fs => let* t := as_u8 "tag" t in let* fs := mapM as_sym fs in Ok (Ctor t fs)
        | _ => arity_at_least_err h 1 a
        end
      else if String.eqb h "field" then
        match a with
        | [x; i] => let* x := as_sym x in let* i := as_u8 "field index" i in Ok (Field x i)
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
            let* op := decode_prim op in let* args := mapM as_sym args in Ok (Prim op args)
        | _ => arity_at_least_err h 1 a
        end
      else if String.eqb h "extern" then
        match a with
        | [n] => fmap Extern (as_u16 "extern slot" n)
        | _ => arity_err h 1 a
        end
      else Err ("unknown cps value form `" ++ h ++ "`")
  | _ => Err ("expected a cps value, found " ++ describe s)
  end

with decode_expr (s : sexp) : result expr :=
  match s with
  | List (Sym h :: a) =>
      if String.eqb h "let" then
        match a with
        | [x; v; b] =>
            let* x := as_sym x in let* v := decode_val v in
            let* b := decode_expr b in Ok (Let x v b)
        | _ => arity_err h 3 a
        end
      else if String.eqb h "letrec" then
        match a with
        | [f; args; k; b; r] =>
            let* f := as_sym f in let* args := sym_list args in let* k := as_sym k in
            let* b := decode_expr b in let* r := decode_expr r in Ok (Letrec f args k b r)
        | _ => arity_err h 5 a
        end
      else if String.eqb h "encore" then
        match a with
        | [f; args; k] =>
            let* f := as_sym f in let* args := sym_list args in let* k := as_sym k in
            Ok (Encore f args k)
        | _ => arity_err h 3 a
        end
      else if String.eqb h "match" then
        match a with
        | sc :: b :: cs =>
            let* sc := as_sym sc in
            let* b := as_u8 "tag" b in
            let* cs := mapM (fun c =>
              match c with
              | List (Sym ch :: ca) =>
                  if String.eqb ch "case" then
                    match ca with
                    | [bs; body] =>
                        let* bs := sym_list bs in let* body := decode_expr body in Ok (bs, body)
                    | _ => arity_err "case" 2 ca
                    end
                  else Err ("expected `(case ...)`, found " ++ describe c)
              | _ => Err ("expected `(case ...)`, found " ++ describe c)
              end) cs in
            Ok (Match sc b cs)
        | _ => arity_at_least_err h 2 a
        end
      else if String.eqb h "fin" then
        match a with
        | [x] => fmap Fin (as_sym x)
        | _ => arity_err h 1 a
        end
      else Err ("unknown cps expression form `" ++ h ++ "`")
  | _ => Err ("expected a cps expression, found " ++ describe s)
  end.

Definition header : string := "encore ir: cps".

Definition encode (m : module) : list sexp :=
  map (fun d => form "define" [Sym (def_name d); encode_expr (def_body d)]) m.

Definition decode (forms : list sexp) : result module :=
  decode_defines (fun n b =>
    let* n := as_sym n in let* b := decode_expr b in
    Ok {| def_name := n; def_body := b |}) forms.
