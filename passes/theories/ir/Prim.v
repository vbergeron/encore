(** Primitive operations, shared by every IR (crates/encore_compiler/src/ir/prim.rs). *)

From Stdlib Require Import String List.
From EncorePasses Require Import Result Sexp.
Import ListNotations.
Open Scope string_scope.

Inductive int_op : Type :=
| Add | Sub | Mul | Eq | Lt | Byte
| Div      (** truncating, [x / 0 = 0] (Rocq [Nat.div]) *)
| Mod      (** truncating, [x mod 0 = x] (Rocq [Nat.modulo]) *)
| SubSat   (** [max(a - b, 0)] (Rocq [Nat.sub]) *)
| Le | And | Or | Xor | Shl
| Shr.     (** logical right shift of the 24-bit payload *)

Inductive bytes_op : Type :=
| Len | Get | Concat | Slice | BEq.

Inductive prim : Type :=
| PInt (op : int_op)
| PBytes (op : bytes_op).

(** Every primitive with its text name, in the order IR.md lists them. *)
Definition prims : list (prim * string) := [
  (PInt Add, "int.add"); (PInt Sub, "int.sub"); (PInt Mul, "int.mul");
  (PInt Eq, "int.eq"); (PInt Lt, "int.lt"); (PInt Byte, "int.byte");
  (PInt Div, "int.div"); (PInt Mod, "int.mod"); (PInt SubSat, "int.sub_sat");
  (PInt Le, "int.le"); (PInt And, "int.and"); (PInt Or, "int.or");
  (PInt Xor, "int.xor"); (PInt Shl, "int.shl"); (PInt Shr, "int.shr");
  (PBytes Len, "bytes.len"); (PBytes Get, "bytes.get");
  (PBytes Concat, "bytes.concat"); (PBytes Slice, "bytes.slice");
  (PBytes BEq, "bytes.eq")
].

Definition prim_name (p : prim) : string :=
  match p with
  | PInt Add => "int.add" | PInt Sub => "int.sub" | PInt Mul => "int.mul"
  | PInt Eq => "int.eq" | PInt Lt => "int.lt" | PInt Byte => "int.byte"
  | PInt Div => "int.div" | PInt Mod => "int.mod" | PInt SubSat => "int.sub_sat"
  | PInt Le => "int.le" | PInt And => "int.and" | PInt Or => "int.or"
  | PInt Xor => "int.xor" | PInt Shl => "int.shl" | PInt Shr => "int.shr"
  | PBytes Len => "bytes.len" | PBytes Get => "bytes.get"
  | PBytes Concat => "bytes.concat" | PBytes Slice => "bytes.slice"
  | PBytes BEq => "bytes.eq"
  end.

(** The table and [prim_name] agree. *)
Lemma prims_names : forall p n, In (p, n) prims -> prim_name p = n.
Proof. simpl; intros p n H; repeat (destruct H as [H | H]; [now inversion H |]); destruct H. Qed.

Definition encode_prim (p : prim) : sexp := Sym (prim_name p).

Definition decode_prim (s : sexp) : result prim :=
  let* name := as_sym s in
  match find (fun pn => String.eqb (snd pn) name) prims with
  | Some (p, _) => Ok p
  | None => Err ("unknown primitive `" ++ name ++ "`")
  end.
