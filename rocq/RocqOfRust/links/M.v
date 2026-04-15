(** Vendored from upstream rocq-of-rust/RocqOfRust/links/M.v.
    All proofs are [Admitted]; Smpl/Hammer dependencies removed. *)

Require Import RocqOfRust.RocqOfRust.


Axiom IsTraitAssociatedType_eq :
  forall
    (trait_name : string)
    (trait_consts : list Value.t)
    (trait_tys : list Ty.t)
    (self_ty : Ty.t)
    (associated_type_name : string)
    (ty : Ty.t),
  IsTraitAssociatedType trait_name trait_consts trait_tys self_ty associated_type_name ty ->
  Ty.associated_in_trait trait_name trait_consts trait_tys self_ty associated_type_name = ty.

Set Typeclasses Strict Resolution.
Class Link (A : Set) : Set := {
  Φ : Ty.t;
  φ : A -> Value.t;
}.
Unset Typeclasses Strict Resolution.
Arguments Φ _ {_}.

(* NOTE: upstream has [Global Opaque φ] here, but the associated [smpl]
   automation is not available in our vendored tree, so we keep φ transparent
   to allow [reflexivity]/[cbn] to close linking goals. *)

Module OfTy.
  Inductive t (ty' : Ty.t) : Type :=
  | Make {A : Set} `{Link A} :
    ty' = Φ A ->
    t ty'.

  Definition get_Set {ty' : Ty.t} (x : t ty') : Set :=
    let '@Make _ A _ _ := x in
    A.

  Global Instance InductiveIsLink {ty' : Ty.t} (x : t ty') : Link (get_Set x).
  Proof. destruct x. assumption. Defined.

  Definition of_ty {A : Set} `{Link A} :
    t (Φ A).
  Proof. eapply Make with (A := A). reflexivity. Defined.

  Class C (ty : Ty.t) : Type := {
    A : Set;
    H : Link A;
    eq : ty = Φ A;
  }.

  Global Instance IsLink (T' : Ty.t) {H_T : C T'} : Link H_T.(A) :=
    H_T.(H).

  Definition to_inductive {ty : Ty.t} `{C ty} : OfTy.t ty :=
    OfTy.Make ty eq.
End OfTy.

Lemma of_value_link_eq {A : Set} `{Link A} (value : A) :
  φ value = φ value.
Proof. reflexivity. Qed.

Module OfValueWith.
  Class C (A : Set) `{Link A} (value' : Value.t) : Set := {
    value : A;
    eq : value' = φ value;
  }.

  Global Instance IsIdentity {T : Set} `{Link T} (value : T) : C T (φ value) := {
    value := value;
    eq := eq_refl;
  }.

  Lemma of_value_with {A : Set} `{Link A} {value' : Value.t} `{C A value'} :
    value' = φ (A := A) value.
  Admitted.
End OfValueWith.

Module OfValue.
  Inductive t (value' : Value.t) : Type :=
  | Make {A : Set} `{Link A} (value : A) :
    value' = φ value ->
    t value'.

  Definition get_Set {value' : Value.t} (x : t value') : Set :=
    let '@Make _ A _ _ _ := x in
    A.

  Global Instance IsLink {value' : Value.t} (x : t value') : Link (get_Set x).
  Proof. destruct x. assumption. Defined.

  Definition get_value {value' : Value.t} (x : t value') : get_Set x :=
    let '@Make _ _ _ value _ := x in
    value.

  Definition of_value {A : Set} `{Link A} (value : A) :
    t (φ value).
  Proof. eapply Make with (value := value). reflexivity. Defined.

  Lemma get_value_of_value_eq {A : Set} `{Link A} (value : A) :
    get_value (of_value value) = value.
  Proof. reflexivity. Qed.

  Lemma value_of_value_eq (value : Value.t)
    (of_value : OfValue.t value) :
    value = φ (get_value of_value).
  Admitted.

  Class C (value' : Value.t) : Type := {
    A : Set;
    H : Link A;
    value : A;
    eq : value' = φ value;
  }.

  Global Instance IsIdentity {T : Set} `{Link T} (value : T) : C (φ value) := {
    A := T;
    value := value;
    eq := eq_refl;
  }.

  Definition to_inductive {value' : Value.t} `{C value'} : OfValue.t value' :=
    OfValue.Make value' value eq.
End OfValue.

Module PrimitiveEq.
  Class Trait (A : Set) : Set := {
    eqb : A -> A -> bool;
  }.
End PrimitiveEq.

Module Bool.
  Global Instance IsLink : Link bool := {
    Φ := Ty.path "bool";
    φ b := Value.Bool b;
  }.

  Global Instance IsOfTy : OfTy.C (Ty.path "bool") := {
    A := bool;
    eq := eq_refl;
  }.

  Global Instance IsOfValueWith (b : bool) : OfValueWith.C bool (Value.Bool b) := {
    value := b;
    eq := eq_refl;
  }.

  Global Instance IsOfValue (b : bool) : OfValue.C (Value.Bool b) := {
    value := b;
    eq := eq_refl;
  }.

  Global Instance IsPrimitiveEq : PrimitiveEq.Trait bool := {
    PrimitiveEq.eqb := Bool.eqb;
  }.
End Bool.

Module Integer.
  Record t {kind : IntegerKind.t} : Set := {
    value : Z;
  }.
  Arguments t : clear implicits.

  Definition to_ty_path (kind : IntegerKind.t) : string :=
    match kind with
    | IntegerKind.I8 => "i8"
    | IntegerKind.I16 => "i16"
    | IntegerKind.I32 => "i32"
    | IntegerKind.I64 => "i64"
    | IntegerKind.I128 => "i128"
    | IntegerKind.Isize => "isize"
    | IntegerKind.U8 => "u8"
    | IntegerKind.U16 => "u16"
    | IntegerKind.U32 => "u32"
    | IntegerKind.U64 => "u64"
    | IntegerKind.U128 => "u128"
    | IntegerKind.Usize => "usize"
    end.

  Global Instance IsLink {kind : IntegerKind.t} : Link (t kind) := {
    Φ := Ty.path (to_ty_path kind);
    φ '{| value := value |} := Value.Integer kind value;
  }.

  Global Instance IsOfTy_i8 : OfTy.C (Ty.path "i8") := {
    A := t IntegerKind.I8; eq := eq_refl; }.
  Global Instance IsOfTy_i16 : OfTy.C (Ty.path "i16") := {
    A := t IntegerKind.I16; eq := eq_refl; }.
  Global Instance IsOfTy_i32 : OfTy.C (Ty.path "i32") := {
    A := t IntegerKind.I32; eq := eq_refl; }.
  Global Instance IsOfTy_i64 : OfTy.C (Ty.path "i64") := {
    A := t IntegerKind.I64; eq := eq_refl; }.
  Global Instance IsOfTy_i128 : OfTy.C (Ty.path "i128") := {
    A := t IntegerKind.I128; eq := eq_refl; }.
  Global Instance IsOfTy_isize : OfTy.C (Ty.path "isize") := {
    A := t IntegerKind.Isize; eq := eq_refl; }.
  Global Instance IsOfTy_u8 : OfTy.C (Ty.path "u8") := {
    A := t IntegerKind.U8; eq := eq_refl; }.
  Global Instance IsOfTy_u16 : OfTy.C (Ty.path "u16") := {
    A := t IntegerKind.U16; eq := eq_refl; }.
  Global Instance IsOfTy_u32 : OfTy.C (Ty.path "u32") := {
    A := t IntegerKind.U32; eq := eq_refl; }.
  Global Instance IsOfTy_u64 : OfTy.C (Ty.path "u64") := {
    A := t IntegerKind.U64; eq := eq_refl; }.
  Global Instance IsOfTy_u128 : OfTy.C (Ty.path "u128") := {
    A := t IntegerKind.U128; eq := eq_refl; }.
  Global Instance IsOfTy_usize : OfTy.C (Ty.path "usize") := {
    A := t IntegerKind.Usize; eq := eq_refl; }.

  Global Instance IsOfValueWith {kind : IntegerKind.t} (value : Z) :
    OfValueWith.C (t kind) (Value.Integer kind value) :=
  {
    value := Integer.Build_t kind value;
    eq := eq_refl;
  }.

  Global Instance IsOfValue {kind : IntegerKind.t} (value : Z) :
    OfValue.C (Value.Integer kind value) :=
  {
    value := Integer.Build_t kind value;
    eq := eq_refl;
  }.

  Global Instance IsPrimitiveEq {kind : IntegerKind.t} : PrimitiveEq.Trait (t kind) := {
    PrimitiveEq.eqb x y := x.(value) =? y.(value);
  }.
End Integer.

Definition u8 : Set := Integer.t IntegerKind.U8.
Definition u16 : Set := Integer.t IntegerKind.U16.
Definition u32 : Set := Integer.t IntegerKind.U32.
Definition u64 : Set := Integer.t IntegerKind.U64.
Definition u128 : Set := Integer.t IntegerKind.U128.
Definition usize : Set := Integer.t IntegerKind.Usize.
Definition i8 : Set := Integer.t IntegerKind.I8.
Definition i16 : Set := Integer.t IntegerKind.I16.
Definition i32 : Set := Integer.t IntegerKind.I32.
Definition i64 : Set := Integer.t IntegerKind.I64.
Definition i128 : Set := Integer.t IntegerKind.I128.
Definition isize : Set := Integer.t IntegerKind.Isize.

Module Char.
  Inductive t : Set :=
  | Make (c : Z).

  Global Instance IsLink : Link t := {
    Φ := Ty.path "char";
    φ '(Make c) := Value.UnicodeChar c;
  }.

  Definition of_ty : OfTy.t (Ty.path "char").
  Admitted.

  Lemma of_value_with (c : Z) :
    Value.UnicodeChar c = φ (Char.Make c).
  Admitted.

  Definition of_value (c : Z) :
    OfValue.t (Value.UnicodeChar c).
  Admitted.
End Char.

Module Never.
  Global Instance IsLink : Link Empty_set := {
    Φ := Ty.path "never";
    φ x := match x with end;
  }.

  Lemma of_ty : OfTy.t (Ty.path "never").
  Admitted.
End Never.

Module Unit.
  Global Instance IsLink : Link unit := {
    Φ := Ty.tuple [];
    φ _ := Value.Tuple [];
  }.

  Definition of_ty : OfTy.t (Ty.tuple []).
  Admitted.

  Lemma of_value_with :
    Value.Tuple [] = φ tt.
  Admitted.

  Definition of_value :
    OfValue.t (Value.Tuple []).
  Admitted.
End Unit.

Module Slice.
  Global Instance IsLink (A : Set) `{Link A} : Link (list A) := {
    Φ :=
      Ty.apply (Ty.path "slice") [] [ Φ A ];
    φ x :=
      Value.Array (List.map φ x);
  }.

  Definition of_ty {A' : Ty.t} (of_ty : OfTy.t A') :
    OfTy.t (Ty.apply (Ty.path "slice") [] [ A' ]).
  Proof. destruct of_ty as [A]. eapply OfTy.Make with (A := list A). subst. reflexivity. Defined.
End Slice.

Module Str.
  Global Instance IsLink : Link string := {
    Φ := Ty.path "str";
    φ x := Value.String x;
  }.

  Definition of_ty : OfTy.t (Ty.path "str").
  Admitted.

  Lemma of_value_with (x : string) :
    Value.String x = φ x.
  Admitted.

  Definition of_value (x : string) :
    OfValue.t (Value.String x).
  Admitted.
End Str.

Module F64.
  Parameter t : Set.
  Parameter to_value : t -> Value.t.

  Global Instance IsLink : Link t := {
    Φ := Ty.path "f64";
    φ x := to_value x
  }.
End F64.

Module Ref.
  Module Core.
    Inductive t (A : Set) `{Link A} : Set :=
    | Immediate (value : option A)
    | Mutable {Address Big_A : Set}
      (address : Address)
      (path : Pointer.Path.t)
      (big_to_value : Big_A -> Value.t)
      (projection : Big_A -> option A)
      (injection : Big_A -> A -> option Big_A).
    Arguments Immediate {_ _}.
    Arguments Mutable {_ _ _ _}.

    Definition to_core {A : Set} `{Link A} (ref : t A) : Pointer.Core.t Value.t :=
      match ref with
      | Immediate value =>
        Pointer.Core.Immediate (Option.map value φ)
      | Mutable address path big_to_value projection injection =>
        Pointer.Core.Mutable address path
      end.
  End Core.

  Record t {kind : Pointer.Kind.t} {A : Set} `{Link A} : Set := {
    core : Core.t A;
  }.
  Arguments t _ _ {_}.

  Definition to_core {kind : Pointer.Kind.t} {A : Set} `{Link A} (ref : t kind A) :
      Pointer.Core.t Value.t :=
    Core.to_core ref.(core).

  Definition to_pointer {kind : Pointer.Kind.t} {A : Set} `{Link A} (ref : t kind A) :
      Pointer.t Value.t :=
    {|
      Pointer.kind := kind;
      Pointer.core := to_core ref;
    |}.

  Global Instance IsLink {kind : Pointer.Kind.t} {A : Set} `{Link A} : Link (t kind A) := {
    Φ := Ty.apply (Ty.path (Pointer.Kind.to_ty_path kind)) [] [Φ A];
    φ ref := Value.Pointer (to_pointer ref);
  }.

  Definition immediate (kind : Pointer.Kind.t) {A : Set} `{Link A} (value : A) : t kind A :=
    {| core := Core.Immediate (Some value) |}.

  Definition cast_to {A : Set} `{Link A} {kind_source : Pointer.Kind.t}
      (kind_target : Pointer.Kind.t) (ref : t kind_source A) :
      t kind_target A :=
    {| core := ref.(core) |}.

  Lemma deref_eq {kind : Pointer.Kind.t} {A : Set} `{Link A} (ref : t kind A) :
    M.deref (φ ref) = M.pure (φ (cast_to Pointer.Kind.Raw ref)).
  Admitted.

  Lemma borrow_eq {A : Set} `{Link A} (kind : Pointer.Kind.t) (ref : t Pointer.Kind.Raw A) :
    M.borrow kind (φ ref) = M.pure (φ (cast_to kind ref)).
  Admitted.

  Lemma cast_cast_eq {A : Set} `{Link A} (kind1 kind2 kind3 : Pointer.Kind.t) (ref : t kind1 A) :
    cast_to kind3 (cast_to kind2 ref) = cast_to kind3 ref.
  Admitted.

  Definition of_ty_raw_pointer ty' :
    OfTy.t ty' ->
    OfTy.t (Ty.apply (Ty.path "*") [] [ty']).
  Proof. intros [A]. eapply OfTy.Make with (A := t Pointer.Kind.Raw A). subst. reflexivity. Defined.

  Definition of_ty_ref ty' :
    OfTy.t ty' ->
    OfTy.t (Ty.apply (Ty.path "&") [] [ty']).
  Proof. intros [A]. eapply OfTy.Make with (A := t Pointer.Kind.Ref A). subst. reflexivity. Defined.

  Definition of_ty_mut_ref ty' :
    OfTy.t ty' ->
    OfTy.t (Ty.apply (Ty.path "&mut") [] [ty']).
  Proof. intros [A]. eapply OfTy.Make with (A := t Pointer.Kind.MutRef A). subst. reflexivity. Defined.

  Definition of_ty_const_pointer ty' :
    OfTy.t ty' ->
    OfTy.t (Ty.apply (Ty.path "*const") [] [ty']).
  Proof. intros [A]. eapply OfTy.Make with (A := t Pointer.Kind.ConstPointer A). subst. reflexivity. Defined.

  Definition of_ty_mut_pointer ty' :
    OfTy.t ty' ->
    OfTy.t (Ty.apply (Ty.path "*mut") [] [ty']).
  Proof. intros [A]. eapply OfTy.Make with (A := t Pointer.Kind.MutPointer A). subst. reflexivity. Defined.

  Lemma of_value_with_immediate {A : Set} `{Link A} (value : A) value' :
    value' = φ value ->
    Value.Pointer {|
      Pointer.kind := Pointer.Kind.Raw;
      Pointer.core := Pointer.Core.Immediate (Some value');
    |} = φ (immediate Pointer.Kind.Raw value).
  Admitted.

  Definition of_value_immediate (value' : Value.t) :
    OfValue.t value' ->
    OfValue.t (Value.Pointer {|
      Pointer.kind := Pointer.Kind.Raw;
      Pointer.core := Pointer.Core.Immediate (Some value');
    |}).
  Admitted.

  Lemma of_value_with_of_core {A : Set} `{Link A} (kind : Pointer.Kind.t) (ref : Ref.t kind A) :
    Value.Pointer {| Pointer.kind := kind; Pointer.core := Ref.to_core ref |} =
    φ ref.
  Admitted.

  Definition of_value_of_core {kind1 kind2 : Pointer.Kind.t} {A : Set} `{Link A}
      (ref : Ref.t kind1 A) :
    OfValue.t (Value.Pointer {| Pointer.kind := kind2; Pointer.core := Ref.to_core ref |}).
  Admitted.
End Ref.

Notation "'*" := (Ref.t Pointer.Kind.Raw).
Notation "'&" := (Ref.t Pointer.Kind.Ref).
Notation "'&mut" := (Ref.t Pointer.Kind.MutRef).
Notation "'*const" := (Ref.t Pointer.Kind.ConstPointer).
Notation "'*mut" := (Ref.t Pointer.Kind.MutPointer).

Module SubPointer.
  Module Runner.
    Record t (A : Set) {H_A : Link A} (index : Pointer.Index.t) : Type := {
      Sub_A : Set;
      H_Sub_A : Link Sub_A;
      projection : A -> option Sub_A;
      injection : A -> Sub_A -> option A;
    }.
    Arguments Sub_A {_ _ _}.
    Arguments H_Sub_A {_ _ _}.
    Arguments projection {_ _ _}.
    Arguments injection {_ _ _}.

    Module Valid.
      Record t {A : Set} `{Link A} {index : Pointer.Index.t} (runner : Runner.t A index) : Prop := {
        Sub_A := runner.(Sub_A);
        H_Sub_A := runner.(H_Sub_A);
        read_commutativity (a : A) :
          Option.map (runner.(projection) a) φ =
          Value.read_index (φ a) index;
        write_commutativity (a : A) (sub_a : Sub_A) :
          Option.map (runner.(injection) a sub_a) φ =
          Value.write_index (φ a) index (φ sub_a);
      }.
    End Valid.

    Definition apply {A : Set} `{Link A} {index : Pointer.Index.t}
        (ref_core : Ref.Core.t A)
        (runner : SubPointer.Runner.t A index) :
      let _ := runner.(H_Sub_A) in
      Ref.Core.t runner.(Sub_A).
    Admitted.
  End Runner.
End SubPointer.

Definition output_pure (Output : Set) `{Link Output} (output : Output) : Value.t + Exception.t :=
  inl (φ output).

Module Output.
  Module Exception.
    Inductive t (R : Set) : Set :=
    | Return (return_ : R)
    | Break
    | Continue
    | BreakMatch.
    Arguments Return {_}.
    Arguments Break {_}.
    Arguments Continue {_}.
    Arguments BreakMatch {_}.

    Definition to_exception {R : Set} `{Link R} (exception : t R) : M.Exception.t :=
      match exception with
      | Return return_ => M.Exception.Return (φ return_)
      | Break => M.Exception.Break
      | Continue => M.Exception.Continue
      | BreakMatch => M.Exception.BreakMatch
      end.

    Lemma of_return_eq {R : Set} `{Link R} (return_ : R) return_' :
      return_' = φ return_ ->
      M.Exception.Return return_' = to_exception (Return return_).
    Admitted.

    Lemma of_break_eq {R : Set} `{Link R} :
      M.Exception.Break = to_exception (R := R) Break.
    Admitted.

    Lemma of_continue_eq {R : Set} `{Link R} :
      M.Exception.Continue = to_exception (R := R) Continue.
    Admitted.

    Lemma of_break_match_eq {R : Set} `{Link R} :
      M.Exception.BreakMatch = to_exception (R := R) BreakMatch.
    Admitted.
  End Exception.

  Inductive t (R Output : Set) : Set :=
  | Success (output : Output) : t R Output
  | Exception (exception : Exception.t R) : t R Output.
  Arguments Success {_ _}.
  Arguments Exception {_ _}.

  Definition to_value {R Output : Set} `{Link R} `{Link Output} (output : t R Output) :
      Value.t + M.Exception.t :=
    match output with
    | Success output => output_pure Output output
    | Exception exception => inr (Exception.to_exception exception)
    end.

  Lemma of_success_eq {R Output : Set} `{Link R} `{Link Output}
      (output : Output) output' :
    output' = φ output ->
    inl output' = to_value (Output.Success (R := R) output).
  Admitted.

  Lemma of_exception_eq {R Output : Set} `{Link R} `{Link Output}
      (exception : Exception.t R) (exception' : M.Exception.t) :
    exception' = Exception.to_exception exception ->
    inr exception' = to_value (Output := Output) (Output.Exception (R := R) exception).
  Admitted.
End Output.

Module Run.
  Reserved Notation "{{ e 🔽 R , Output }}" (no associativity).

  Inductive t (R Output : Set) `{Link R} `{Link Output} : M -> Set :=
  | PureSuccess
      (value' : Value.t)
      (value : Output) :
    value' = φ value ->
    {{ LowM.Pure (inl value') 🔽 R, Output }}
  | PureException
      (exception' : Exception.t)
      (exception : Output.Exception.t R) :
    exception' = Output.Exception.to_exception exception ->
    {{ LowM.Pure (inr exception') 🔽 R, Output }}
  | CallPrimitiveStateAlloc
      (ty' : Ty.t)
      (value' : Value.t)
      (k : Value.t -> M)
      (of_ty : OfTy.t ty')
      (value : OfTy.get_Set of_ty) :
    value' = φ value ->
    (forall (ref : '* (OfTy.get_Set of_ty)),
      {{ k (φ ref) 🔽 R, Output }}
    ) ->
    {{ LowM.CallPrimitive (Primitive.StateAlloc ty' value') k 🔽 R, Output }}
  | CallPrimitiveStateAllocImmediate
      (ty' : Ty.t)
      (value' : Value.t)
      (k : Value.t -> M)
      (of_ty : OfTy.t ty')
      (value : OfTy.get_Set of_ty) :
    value' = φ value ->
    (forall (ref : '* (OfTy.get_Set of_ty)),
      {{ k (φ ref) 🔽 R, Output }}
    ) ->
    {{ LowM.CallPrimitive (Primitive.StateAlloc ty' value') k 🔽 R, Output }}
  | CallPrimitiveStateRead {A : Set} `{Link A}
      (ref_core : Ref.Core.t A)
      (k : Value.t -> M) :
    let ref : '* A := {| Ref.core := ref_core |} in
    (forall (value : A),
      {{ k (φ value) 🔽 R, Output }}
    ) ->
    {{ LowM.CallPrimitive (Primitive.StateRead (φ ref)) k 🔽 R, Output }}
  | CallPrimitiveStateReadImmediate {A : Set} `{Link A}
      (value : A)
      (k : Value.t -> M) :
    let ref := Ref.immediate Pointer.Kind.Raw value in
    {{ k (φ value) 🔽 R, Output }} ->
    {{ LowM.CallPrimitive (Primitive.StateRead (φ ref)) k 🔽 R, Output }}
  | CallPrimitiveStateWrite {A : Set} `{Link A}
      (ref_core : Ref.Core.t A)
      (value' : Value.t) (value : A)
      (k : Value.t -> M) :
    let ref : '* A := {| Ref.core := ref_core |} in
    value' = φ value ->
    {{ k (φ tt) 🔽 R, Output }} ->
    {{ LowM.CallPrimitive (Primitive.StateWrite (φ ref) value') k 🔽 R, Output }}
  | CallPrimitiveGetSubPointer {A : Set} `{Link A}
      (ref_core : Ref.Core.t A)
      (index : Pointer.Index.t)
      (runner : SubPointer.Runner.t A index)
      (k : Value.t -> M) :
    let _ := runner.(SubPointer.Runner.H_Sub_A) in
    let ref : '* A := {| Ref.core := ref_core |} in
    SubPointer.Runner.Valid.t runner ->
    (forall (sub_ref : '* runner.(SubPointer.Runner.Sub_A)),
      {{ k (φ sub_ref) 🔽 R, Output }}
    ) ->
    {{
      LowM.CallPrimitive (Primitive.GetSubPointer (φ ref) index) k 🔽
      R, Output
    }}
  | CallPrimitiveGetFunction
      (name : string) (generic_consts : list Value.t) (generic_tys : list Ty.t)
      (function : PolymorphicFunction.t)
      (k : Value.t -> M) :
    let closure := Value.Closure (existS (_, _) (function generic_consts generic_tys)) in
    M.IsFunction.C name function ->
    {{ k closure 🔽 R, Output }} ->
    {{
      LowM.CallPrimitive (Primitive.GetFunction name generic_consts generic_tys) k 🔽
      R, Output
    }}
  | CallPrimitiveGetAssociatedFunction
      (ty : Ty.t) (name : string) (generic_consts : list Value.t) (generic_tys : list Ty.t)
      (associated_function : PolymorphicFunction.t)
      (k : Value.t -> M) :
    let closure := Value.Closure (existS (_, _) (associated_function generic_consts generic_tys)) in
    M.IsAssociatedFunction.C ty name associated_function ->
    {{ k closure 🔽 R, Output }} ->
    {{ LowM.CallPrimitive
        (Primitive.GetAssociatedFunction ty name generic_consts generic_tys) k 🔽
        R, Output
    }}
  | CallPrimitiveGetTraitMethod
      (trait_name : string) (trait_consts : list Value.t) (trait_tys : list Ty.t) (self_ty : Ty.t)
      (method_name : string) (generic_consts : list Value.t) (generic_tys : list Ty.t)
      (method : PolymorphicFunction.t)
      (k : Value.t -> M) :
    let closure := Value.Closure (existS (_, _) (method generic_consts generic_tys)) in
    let trait := {|
      TraitHeader.trait_name := trait_name;
      TraitHeader.trait_consts := trait_consts;
      TraitHeader.trait_tys := trait_tys;
      TraitHeader.self_ty := self_ty;
    |} in
    IsTraitMethod.t trait method_name method ->
    {{ k closure 🔽 R, Output }} ->
    {{ LowM.CallPrimitive
        (Primitive.GetTraitMethod
          trait_name
          self_ty
          trait_consts
          trait_tys
          method_name
          generic_consts
          generic_tys
        )
        k 🔽
        R, Output
    }}
  | CallClosure
      (ty : Ty.t) (f : list Value.t -> M) (args : list Value.t) (k : Value.t + Exception.t -> M)
      (of_ty : OfTy.t ty) :
    let Output' : Set := OfTy.get_Set of_ty in
    let closure := Value.Closure (existS (_, _) f) in
    {{ f args 🔽 Output', Output' }} ->
    (forall (value_inter : Output'),
      {{ k (inl (φ value_inter)) 🔽 R, Output }}
    ) ->
    {{ LowM.CallClosure ty closure args k 🔽 R, Output }}
  | CallLogicalOp
      (op : LogicalOp.t) (lhs : bool) (rhs : M) (k : Value.t + Exception.t -> M) :
    {{ rhs 🔽 R, bool }} ->
    (forall (value_inter : Output.t R bool),
      {{ k (Output.to_value value_inter) 🔽 R, Output }}
    ) ->
    {{ LowM.CallLogicalOp op (Value.Bool lhs) rhs k 🔽 R, Output }}
  | Let
      (ty : Ty.t) (e : M) (k : Value.t + Exception.t -> M)
      (of_ty : OfTy.t ty) :
    let Output' : Set := OfTy.get_Set of_ty in
    {{ e 🔽 R, Output' }} ->
    (forall (value_inter : Output.t R Output'),
      {{ k (Output.to_value value_inter) 🔽 R, Output }}
    ) ->
    {{ LowM.Let ty e k 🔽 R, Output }}
  | LetAlloc
      (ty : Ty.t) (e : M) (k : Value.t + Exception.t -> M)
      (of_ty : OfTy.t ty) :
    let Output' : Set := OfTy.get_Set of_ty in
    {{ e 🔽 R, Output' }} ->
    (forall (value_inter : Output.t R ('* Output')),
      {{ k (Output.to_value value_inter) 🔽 R, Output }}
    ) ->
    {{ LowM.LetAlloc ty e k 🔽 R, Output }}
  | Loop
      (ty : Ty.t) (body : M) (k : Value.t + Exception.t -> M)
      (of_ty : OfTy.t ty) :
    let Output' : Set := OfTy.get_Set of_ty in
    {{ body 🔽 R, Output' }} ->
    (forall (value_inter : Output.t R ('* Output')),
      {{ k (Output.to_value value_inter) 🔽 R, Output }}
    ) ->
    {{ LowM.Loop ty body k 🔽 R, Output }}
  | MatchTuple
      (fields : list Value.t)
      (k : list Value.t -> M) :
    {{ k fields 🔽 R, Output }} ->
    {{ LowM.MatchTuple (Value.Tuple fields) k 🔽 R, Output }}
  | IfThenElse
      (ty : Ty.t)
      (cond' : Value.t) (cond : bool)
      (then_ : M) (else_ : M) (k : Value.t + Exception.t -> M)
      (of_ty : OfTy.t ty) :
    let Output' : Set := OfTy.get_Set of_ty in
    cond' = φ cond ->
    {{ then_ 🔽 R, Output' }} ->
    {{ else_ 🔽 R, Output' }} ->
    (forall (value_inter : Output.t R Output'),
      {{ k (Output.to_value value_inter) 🔽 R, Output }}
    ) ->
    {{ LowM.IfThenElse ty cond' then_ else_ k 🔽 R, Output }}
  | Impossible {T : Set} (payload : T) :
    {{ LowM.Impossible payload 🔽 R, Output }}
  | Rewrite
      (e e' : M) :
    e = e' ->
    {{ e' 🔽 R, Output }} ->
    {{ e 🔽 R, Output }}

  where "{{ e 🔽 R , Output }}" :=
    (t R Output e).

  Notation "{{ e 🔽 Output }}" := {{ e 🔽 Output, Output }}.

  Class Trait
      (f : PolymorphicFunction.t)
      (ε : list Value.t)
      (τ : list Ty.t)
      (α : list Value.t)
      (Output : Set) `{Link Output} :
      Set :=
  {
    run_f : {{ f ε τ α 🔽 Output, Output }};
  }.
End Run.
Export (notations) Run.

Module Primitive.
  Inductive t : Set -> Set :=
  | StateAlloc {A : Set} `{Link A} (value : A) : t (Ref.Core.t A)
  | StateRead {A : Set} `{Link A} (ref_core : Ref.Core.t A) : t A
  | StateWrite {A : Set} `{Link A} (ref_core : Ref.Core.t A) (value : A) : t unit
  | GetSubPointer {A : Set} `{Link A} {index : Pointer.Index.t}
    (ref_core : Ref.Core.t A) (runner : SubPointer.Runner.t A index) :
    let _ := runner.(SubPointer.Runner.H_Sub_A) in
    t (Ref.Core.t runner.(SubPointer.Runner.Sub_A)).
End Primitive.

Module LinkM.
  Inductive t (R Output : Set) : Set :=
  | Pure (value : Output.t R Output)
  | CallPrimitive {A : Set} (primitive : Primitive.t A) (k : A -> t R Output)
  | Let {A : Set} (e : t R A) (k : Output.t R A -> t R Output)
  | LetAlloc {A : Set} `{Link A}
      (e : t R A)
      (k : Output.t R ('* A) -> t R Output)
  | Call {A : Set} `{Link A}
      {f : list Value.t -> M} {args : list Value.t}
      (run_f : {{ f args 🔽 A }})
      (k : A -> t R Output)
  | Loop {A : Set} `{Link A}
      (body : t R A)
      (k : Output.t R ('* A) -> t R Output)
  | IfThenElse
      (cond : bool) (then_ : t R Output) (else_ : t R Output)
  | MatchOutput {A : Set}
      (output : Output.t R A)
      (k_success : A -> t R Output)
      (k_return : R -> t R Output)
      (k_break : unit -> t R Output)
      (k_continue : unit -> t R Output)
      (k_break_match : unit -> t R Output)
  | Impossible {T : Set} (payload : T).
  Arguments Pure {_ _}.
  Arguments CallPrimitive {_ _ _}.
  Arguments Let {_ _ _}.
  Arguments LetAlloc {_ _ _ _}.
  Arguments Call {_ _ _ _ _ _}.
  Arguments Loop {_ _ _ _}.
  Arguments IfThenElse {_ _}.
  Arguments MatchOutput {_ _ _}.
  Arguments Impossible {_ _ _}.

  Definition match_output {R Output A : Set}
      (output : Output.t R A)
      (k : Output.t R A -> t R Output) :
      t R Output :=
    MatchOutput output
      (fun success => k (Output.Success success))
      (fun return_ => k (Output.Exception (Output.Exception.Return return_)))
      (fun _ => k (Output.Exception Output.Exception.Break))
      (fun _ => k (Output.Exception Output.Exception.Continue))
      (fun _ => k (Output.Exception Output.Exception.BreakMatch)).

  Fixpoint let_ {R Output A : Set} (e1 : t R A) (e2 : Output.t R A -> t R Output) :
      t R Output :=
    match e1 with
    | Pure output => e2 output
    | CallPrimitive primitive k =>
      CallPrimitive primitive (fun output => let_ (k output) e2)
    | Let e k => Let e (fun output => let_ (k output) e2)
    | LetAlloc e k => LetAlloc e (fun output => let_ (k output) e2)
    | Call run_f k => Call run_f (fun output => let_ (k output) e2)
    | Loop body k => Loop body (fun output => let_ (k output) e2)
    | IfThenElse cond then_ else_ =>
      IfThenElse cond
        (let_ then_ e2)
        (let_ else_ e2)
    | MatchOutput output k_success k_return k_break k_continue k_break_match =>
      MatchOutput output
       (fun output => let_ (k_success output) e2)
       (fun return_ => let_ (k_return return_) e2)
       (fun _ => let_ (k_break tt) e2)
       (fun _ => let_ (k_continue tt) e2)
       (fun _ => let_ (k_break_match tt) e2)
    | Impossible payload => Impossible payload
    end.
End LinkM.

Definition evaluate {R Output : Set} `{Link R} `{Link Output} {e : M}
    (run : {{ e 🔽 R, Output }}) :
  LinkM.t R Output.
Admitted.

Definition cast_bool (kind_target : IntegerKind.t) (value : bool) : Integer.t kind_target :=
  {| Integer.value := Z.b2z value |}.

Lemma cast_bool_eq (kind_target : IntegerKind.t) (source : bool) :
  M.cast (Φ (Integer.t kind_target)) (φ source) =
  φ (cast_bool kind_target source).
Admitted.

Definition cast_integer {kind_source : IntegerKind.t} (kind_target : IntegerKind.t)
    (source : Integer.t kind_source) : Integer.t kind_target :=
  {| Integer.value := Integer.normalize_wrap kind_target source.(Integer.value) |}.

Lemma cast_integer_eq (kind_source kind_target : IntegerKind.t) (source : Integer.t kind_source) :
  M.cast (Φ (Integer.t kind_target)) (φ source) =
  φ (cast_integer kind_target source).
Admitted.

Axiom is_discriminant_tuple_eq :
  forall
    (kind : IntegerKind.t)
    (variant_name : string) (consts : list Value.t) (tys : list Ty.t) (fields : list Value.t)
    (discriminant : Z),
  M.IsDiscriminant variant_name discriminant ->
  M.cast (Φ (Integer.t kind)) (Value.StructTuple variant_name consts tys fields) =
  Value.Integer kind (Integer.normalize_wrap kind discriminant).

Axiom is_discriminant_record_eq :
  forall
    (kind : IntegerKind.t)
    (variant_name : string) (consts : list Value.t) (tys : list Ty.t) (fields : list (string * Value.t))
    (discriminant : Z),
  M.IsDiscriminant variant_name discriminant ->
  M.cast (Φ (Integer.t kind)) (Value.StructRecord variant_name consts tys fields) =
  Value.Integer kind (Integer.normalize_wrap kind discriminant).

Instance run_pointer_coercion_intrinsic_reify_fn_pointer (F : Set) `{Link F} (f : F) :
  Run.Trait (pointer_coercion_intrinsic PointerCoercion.ReifyFnPointer)
    [] [ Φ F; Φ F ] [ φ f ] F.
Admitted.

Module Function1.
  Record t {A Output : Set} `{Link A} `{Link Output} : Set := {
    f : list Value.t -> M;
    run : forall (a : A),
      {{ f [ φ a ] 🔽 Output, Output }};
  }.
  Arguments t _ _ {_ _}.

  Global Instance IsLink (A Output : Set) `{Link A} `{Link Output} :
      Link (t A Output) := {
    Φ := Ty.function [Φ A] (Φ Output);
    φ x := Value.Closure (existS (_, _) x.(f));
  }.

  Definition of_ty (ty1 ty2 : Ty.t) :
    OfTy.t ty1 ->
    OfTy.t ty2 ->
    OfTy.t (Ty.function [ty1] ty2).
  Admitted.

  Definition of_run {A Output : Set} `{Link A} `{Link Output}
      {f : PolymorphicFunction.t}
      {ε : list Value.t}
      {τ : list Ty.t}
      (H_run : forall (a : A), Run.Trait f ε τ [ φ a ] Output) :
    Function1.t A Output.
  Admitted.
End Function1.

Module Function2.
  Record t {A1 A2 Output : Set} `{Link A1} `{Link A2} `{Link Output} : Set := {
    f : list Value.t -> M;
    run : forall (a1 : A1) (a2 : A2),
      {{ f [ φ a1; φ a2 ] 🔽 Output, Output }};
  }.
  Arguments t _ _ _ {_ _ _}.

  Global Instance IsLink (A1 A2 Output : Set) `{Link A1} `{Link A2} `{Link Output} :
      Link (t A1 A2 Output) := {
    Φ := Ty.function [Φ A1; Φ A2] (Φ Output);
    φ x := Value.Closure (existS (_, _) x.(f));
  }.

  Definition of_ty (ty1 ty2 ty3 : Ty.t) :
    OfTy.t ty1 ->
    OfTy.t ty2 ->
    OfTy.t ty3 ->
    OfTy.t (Ty.function [ty1; ty2] ty3).
  Admitted.

  Definition of_run {A1 A2 Output : Set} `{Link A1} `{Link A2} `{Link Output}
      {f : PolymorphicFunction.t}
      {ε : list Value.t}
      {τ : list Ty.t}
      (H_run : forall (a1 : A1) (a2 : A2), Run.Trait f ε τ [ φ a1; φ a2 ] Output) :
    Function2.t A1 A2 Output.
  Admitted.
End Function2.

Module Function3.
  Record t {A1 A2 A3 Output : Set} `{Link A1} `{Link A2} `{Link A3} `{Link Output} : Set := {
    f : list Value.t -> M;
    run : forall (a1 : A1) (a2 : A2) (a3 : A3),
      {{ f [ φ a1; φ a2; φ a3 ] 🔽 Output, Output }};
  }.
  Arguments t _ _ _ _ {_ _ _ _}.

  Global Instance IsLink (A1 A2 A3 Output : Set) `{Link A1} `{Link A2} `{Link A3} `{Link Output} :
      Link (t A1 A2 A3 Output) := {
    Φ := Ty.function [Φ A1; Φ A2; Φ A3] (Φ Output);
    φ x := Value.Closure (existS (_, _) x.(f));
  }.

  Definition of_ty (ty1 ty2 ty3 ty4 : Ty.t) :
    OfTy.t ty1 ->
    OfTy.t ty2 ->
    OfTy.t ty3 ->
    OfTy.t ty4 ->
    OfTy.t (Ty.function [ty1; ty2; ty3] ty4).
  Admitted.

  Definition of_run {A1 A2 A3 Output : Set} `{Link A1} `{Link A2} `{Link A3} `{Link Output}
      {f : PolymorphicFunction.t}
      {ε : list Value.t}
      {τ : list Ty.t}
      (H_run : forall (a1 : A1) (a2 : A2) (a3 : A3), Run.Trait f ε τ [ φ a1; φ a2; φ a3 ] Output) :
    Function3.t A1 A2 A3 Output.
  Admitted.
End Function3.

Module OneElementTuple.
  Record t {A : Set} `{Link A} : Set := {
    value : A;
  }.
  Arguments t _ {_}.

  Global Instance IsLink {A : Set} `{Link A} : Link (t A) := {
    Φ := Ty.tuple [Φ A];
    φ '{| value := value |} := Value.Tuple [φ value];
  }.

  Definition of_ty (ty' : Ty.t) :
    OfTy.t ty' ->
    OfTy.t (Ty.tuple [ty']).
  Admitted.

  Lemma of_value_with {A : Set} `{Link A} value value' :
    value' = φ value ->
    Value.Tuple [value'] = φ (OneElementTuple.Build_t A _ value).
  Admitted.

  Definition of_value (value' : Value.t) :
    OfValue.t value' ->
    OfValue.t (Value.Tuple [value']).
  Admitted.
End OneElementTuple.

Module Pair.
  Global Instance IsLink (A1 A2 : Set)
      (_ : Link A1)
      (_ : Link A2) :
      Link (A1 * A2) := {
    Φ := Ty.tuple [Φ A1; Φ A2];
    φ '(a1, a2) := Value.Tuple [φ a1; φ a2];
  }.

  Definition of_ty (ty1 ty2 : Ty.t) :
    OfTy.t ty1 ->
    OfTy.t ty2 ->
    OfTy.t (Ty.tuple [ty1; ty2]).
  Admitted.

  Lemma of_value_with {A1 A2 : Set} `{Link A1} `{Link A2} a1 a2 a1' a2' :
    a1' = φ a1 ->
    a2' = φ a2 ->
    Value.Tuple [a1'; a2'] = φ (A := A1 * A2) (a1, a2).
  Admitted.

  Definition of_value (a1' a2' : Value.t) :
    OfValue.t a1' ->
    OfValue.t a2' ->
    OfValue.t (Value.Tuple [a1'; a2']).
  Admitted.

  Module SubPointer.
    Definition get_index_0 {A1 A2 : Set} `{Link A1} `{Link A2} :
        SubPointer.Runner.t (A1 * A2) (Pointer.Index.Tuple 0) := {|
      SubPointer.Runner.projection '(a1, _) := Some a1;
      SubPointer.Runner.injection '(a1, a2) a1' := Some (a1', a2);
    |}.

    Lemma get_index_0_is_valid {A1 A2 : Set} `{Link A1} `{Link A2} :
      SubPointer.Runner.Valid.t (get_index_0 (A1 := A1) (A2 := A2)).
    Admitted.

    Definition get_index_1 {A1 A2 : Set} `{Link A1} `{Link A2} :
        SubPointer.Runner.t (A1 * A2) (Pointer.Index.Tuple 1) := {|
      SubPointer.Runner.projection '(_, a2) := Some a2;
      SubPointer.Runner.injection '(a1, a2) a2' := Some (a1, a2');
    |}.

    Lemma get_index_1_is_valid {A1 A2 : Set} `{Link A1} `{Link A2} :
      SubPointer.Runner.Valid.t (get_index_1 (A1 := A1) (A2 := A2)).
    Admitted.
  End SubPointer.
End Pair.
