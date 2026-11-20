(** [dsi]: direct style, de Bruijn indexed (crates/encore_compiler/src/ir/dsi.rs).
    Stage [dsi-resolve].

    Index 0 is the innermost binder. Globals sit below every local binder, in
    define order. [Letrec] binds [f] and then its parameter inside its body,
    and only [f] inside the rest. *)

From Stdlib Require Import String List ZArith.
From EncorePasses Require Import Result Sexp Prim.
Import ListNotations.
Open Scope string_scope.

Inductive expr : Type :=
| Var (idx : nat)
| Lambda (arity : nat) (body : expr)
| App (f : expr) (args : list expr)
| Let (bound body : expr)
| Letrec (fun_body rest : expr)
| Ctor (tag : nat) (fields : list expr)
| Field (e : expr) (idx : nat)
| Match (scrut : expr) (base : nat) (cases : list (nat * expr))   (** (arity, body) *)
| EInt (z : Z)
| Bytes (b : string)
| Prim (op : prim) (args : list expr)
| Extern (slot : nat).

Record define : Type := { def_name : string; def_body : expr }.
Definition module : Type := list define.

(** ** Text form *)

Fixpoint encode_expr (e : expr) : sexp :=
  match e with
  | Var i => form "var" [nat_sexp i]
  | EInt z => Int z
  | Lambda n b => form "lambda" [nat_sexp n; encode_expr b]
  | App f args => form "app" (encode_expr f :: map encode_expr args)
  | Let e1 e2 => form "let" [encode_expr e1; encode_expr e2]
  | Letrec b r => form "letrec" [encode_expr b; encode_expr r]
  | Ctor t fs => form "ctor" (nat_sexp t :: map encode_expr fs)
  | Field e i => form "field" [encode_expr e; nat_sexp i]
  | Match s b cs =>
      form "match" (encode_expr s :: nat_sexp b
        :: map (fun c => form "case" [nat_sexp (fst c); encode_expr (snd c)]) cs)
  | Bytes b => form "bytes" [Str b]
  | Prim op args => form "prim" (encode_prim op :: map encode_expr args)
  | Extern n => form "extern" [nat_sexp n]
  end.

Fixpoint decode_expr (s : sexp) : result expr :=
  match s with
  | Int _ => fmap EInt (as_i32 s)
  | List (Sym h :: a) =>
      if String.eqb h "var" then
        match a with
        | [i] => fmap Var (as_index "de Bruijn index" i)
        | _ => arity_err h 1 a
        end
      else if String.eqb h "lambda" then
        match a with
        | [n; b] => let* n := as_index "arity" n in let* b := decode_expr b in Ok (Lambda n b)
        | _ => arity_err h 2 a
        end
      else if String.eqb h "app" then
        match a with
        | f :: args =>
            let* f := decode_expr f in let* args := mapM decode_expr args in Ok (App f args)
        | _ => arity_at_least_err h 1 a
        end
      else if String.eqb h "let" then
        match a with
        | [e1; e2] => let* e1 := decode_expr e1 in let* e2 := decode_expr e2 in Ok (Let e1 e2)
        | _ => arity_err h 2 a
        end
      else if String.eqb h "letrec" then
        match a with
        | [b; r] => let* b := decode_expr b in let* r := decode_expr r in Ok (Letrec b r)
        | _ => arity_err h 2 a
        end
      else if String.eqb h "ctor" then
        match a with
        | t :: fs =>
            let* t := as_u8 "tag" t in let* fs := mapM decode_expr fs in Ok (Ctor t fs)
        | _ => arity_at_least_err h 1 a
        end
      else if String.eqb h "field" then
        match a with
        | [e; i] =>
            let* e := decode_expr e in let* i := as_u8 "field index" i in Ok (Field e i)
        | _ => arity_err h 2 a
        end
      else if String.eqb h "match" then
        match a with
        | sc :: b :: cs =>
            let* sc := decode_expr sc in
            let* b := as_u8 "tag" b in
            let* cs := mapM (fun c =>
              match c with
              | List (Sym ch :: ca) =>
                  if String.eqb ch "case" then
                    match ca with
                    | [n; body] =>
                        let* n := as_index "arity" n in
                        let* body := decode_expr body in Ok (n, body)
                    | _ => arity_err "case" 2 ca
                    end
                  else Err ("expected `(case ...)`, found " ++ describe c)
              | _ => Err ("expected `(case ...)`, found " ++ describe c)
              end) cs in
            Ok (Match sc b cs)
        | _ => arity_at_least_err h 2 a
        end
      else if String.eqb h "bytes" then
        match a with
        | [b] => fmap Bytes (as_str b)
        | _ => arity_err h 1 a
        end
      else if String.eqb h "prim" then
        match a with
        | op :: args =>
            let* op := decode_prim op in let* args := mapM decode_expr args in Ok (Prim op args)
        | _ => arity_at_least_err h 1 a
        end
      else if String.eqb h "extern" then
        match a with
        | [n] => fmap Extern (as_u16 "extern slot" n)
        | _ => arity_err h 1 a
        end
      else Err ("unknown dsi form `" ++ h ++ "`")
  | _ => Err ("expected a dsi expression, found " ++ describe s)
  end.

Definition header : string := "encore ir: dsi".

Definition encode (m : module) : list sexp :=
  map (fun d => form "define" [Sym (def_name d); encode_expr (def_body d)]) m.

Definition decode (forms : list sexp) : result module :=
  decode_defines (fun n b =>
    let* n := as_sym n in let* b := decode_expr b in
    Ok {| def_name := n; def_body := b |}) forms.
