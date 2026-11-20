(** The S-expression tree shared by every IR text format (see IR.md).

    Reading text into [sexp] and printing [sexp] back to text is done on the
    OCaml side (bin/sexp_text.ml), byte-for-byte like the Rust printer in
    crates/encore_compiler/src/ir/text/sexp.rs. Everything between the tree
    and the typed IRs — decoding, encoding, passes — lives here in Rocq. *)

From Stdlib Require Import String Ascii List ZArith.
From EncorePasses Require Import Result.
Import ListNotations.
Open Scope string_scope.

Inductive sexp : Type :=
| Sym (s : string)
| Int (z : Z)
| Str (bytes : string)            (** byte string: one [ascii] per byte *)
| List (items : list sexp).

(** [(head args ...)] *)
Definition form (head : string) (args : list sexp) : sexp :=
  List (Sym head :: args).

Definition syms (xs : list string) : list sexp := map Sym xs.

Definition nat_sexp (n : nat) : sexp := Int (Z.of_nat n).

(** ** Decoding helpers (mirroring the Rust [Node] API and its messages) *)

Definition describe (s : sexp) : string :=
  match s with
  | Sym x => "symbol `" ++ x ++ "`"
  | Int _ => "integer"
  | Str _ => "string"
  | List (Sym h :: _) => "`(" ++ h ++ " ...)`"
  | List _ => "list"
  end.

Definition as_sym (s : sexp) : result string :=
  match s with
  | Sym x => Ok x
  | _ => Err ("expected a symbol, found " ++ describe s)
  end.

Definition as_int (s : sexp) : result Z :=
  match s with
  | Int z => Ok z
  | _ => Err ("expected an integer, found " ++ describe s)
  end.

Definition as_str (s : sexp) : result string :=
  match s with
  | Str b => Ok b
  | _ => Err ("expected a string, found " ++ describe s)
  end.

Definition as_list (s : sexp) : result (list sexp) :=
  match s with
  | List l => Ok l
  | _ => Err ("expected a list, found " ++ describe s)
  end.

Definition sym_list (s : sexp) : result (list string) :=
  let* l := as_list s in mapM as_sym l.

Definition as_form (s : sexp) : option (string * list sexp) :=
  match s with
  | List (Sym h :: args) => Some (h, args)
  | _ => None
  end.

Definition out_of_range {A} (what : string) : result A :=
  Err (what ++ " out of range").

(** A non-negative integer that must be [<= max]: tags are [u8], extern
    slots [u16], ... *)
Definition as_nat (what : string) (max : Z) (s : sexp) : result nat :=
  let* z := as_int s in
  if andb (0 <=? z)%Z (z <=? max)%Z then Ok (Z.to_nat z) else out_of_range what.

(** A non-negative integer with no upper bound ([usize] on the Rust side). *)
Definition as_index (what : string) (s : sexp) : result nat :=
  let* z := as_int s in
  if (0 <=? z)%Z then Ok (Z.to_nat z) else out_of_range what.

(** Integer literals are [i32]. *)
Definition as_i32 (s : sexp) : result Z :=
  let* z := as_int s in
  if andb (- 2147483648 <=? z)%Z (z <=? 2147483647)%Z then Ok z
  else out_of_range "integer literal".

Definition u8_max : Z := 255.
Definition u16_max : Z := 65535.

Definition as_u8 (what : string) := as_nat what u8_max.
Definition as_u16 (what : string) := as_nat what u16_max.

(** Decimal rendering, for error messages and register names. *)
Fixpoint digits (fuel n : nat) (acc : string) : string :=
  match fuel with
  | 0 => acc
  | S fuel =>
      let acc := String (Ascii.ascii_of_nat (48 + Nat.modulo n 10)) acc in
      let q := Nat.div n 10 in
      if Nat.eqb q 0 then acc else digits fuel q acc
  end.

Definition nat_to_string (n : nat) : string := digits (S n) n "".

Definition arity_err {A} (head : string) (n : nat) (args : list sexp) : result A :=
  Err ("`" ++ head ++ "` expects " ++ nat_to_string n
       ++ " argument(s), found " ++ nat_to_string (length args)).

Definition arity_at_least_err {A} (head : string) (n : nat) (args : list sexp) : result A :=
  Err ("`" ++ head ++ "` expects at least " ++ nat_to_string n
       ++ " argument(s), found " ++ nat_to_string (length args)).

(** A file is a sequence of [(define NAME BODY)] forms. *)
Definition decode_defines {D} (define : sexp -> sexp -> result D)
    (forms : list sexp) : result (list D) :=
  mapM (fun f =>
    match as_form f with
    | Some (h, args) =>
        if String.eqb h "define" then
          match args with
          | [name; body] => define name body
          | _ => arity_err "define" 2 args
          end
        else Err ("expected `(define ...)`, found " ++ describe f)
    | None => Err ("expected `(define ...)`, found " ++ describe f)
    end) forms.
