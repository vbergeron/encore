(** * Linking proofs for [encore_vm::arena::Arena].

    This module connects the translated monadic code in [encore_vm.arena]
    to the typed linking layer in [RocqOfRust.links.M], establishing that
    each Arena method is well-typed in the [Run.t] sense. *)

Require Import RocqOfRust.links.RocqOfRust.
Require Import encore_vm.arena.

(** We axiomatize a [Link] for [encore_vm::value::Value] since its
    linking module has not been defined yet. *)
Parameter EncoreValue : Set.
Declare Instance EncoreValueLink : Link EncoreValue.
Axiom EncoreValueLink_Φ :
  Φ EncoreValue = Ty.path "encore_vm::value::Value".

(* ----------------------------------------------------------------- *)
(** ** HeapAddress link type *)

Module HeapAddressL.
  Record t : Set := mk { raw : u16 }.
End HeapAddressL.

Global Instance HeapAddressIsLink : Link HeapAddressL.t := {
  Φ := Ty.path "encore_vm::value::HeapAddress";
  φ a := Value.StructTuple "encore_vm::value::HeapAddress" [] []
    [φ a.(HeapAddressL.raw)];
}.

Definition ha_field0_runner :
    SubPointer.Runner.t HeapAddressL.t
      (Pointer.Index.StructTuple "encore_vm::value::HeapAddress" 0) := {|
  SubPointer.Runner.Sub_A := u16;
  SubPointer.Runner.H_Sub_A := Integer.IsLink;
  SubPointer.Runner.projection a := Some a.(HeapAddressL.raw);
  SubPointer.Runner.injection a raw' := Some (HeapAddressL.mk raw');
|}.

Lemma ha_field0_runner_valid :
  SubPointer.Runner.Valid.t ha_field0_runner.
Proof. constructor; intro a; cbn; reflexivity. Qed.

(* ----------------------------------------------------------------- *)
(** ** Arena link type *)

Module ArenaL.
  Record t : Set := mk {
    hp : usize;
    mem : '&mut (list EncoreValue);
  }.
End ArenaL.

Global Instance ArenaIsLink : Link ArenaL.t := {
  Φ := Ty.path "encore_vm::arena::Arena";
  φ a := Value.mkStructRecord
    "encore_vm::arena::Arena" [] []
    [("mem", φ a.(ArenaL.mem)); ("hp", φ a.(ArenaL.hp))];
}.

(* ----------------------------------------------------------------- *)
(** ** SubPointer runners for Arena fields *)

Definition arena_hp_runner :
    SubPointer.Runner.t ArenaL.t
      (Pointer.Index.StructRecord "encore_vm::arena::Arena" "hp") := {|
  SubPointer.Runner.Sub_A := usize;
  SubPointer.Runner.H_Sub_A := Integer.IsLink;
  SubPointer.Runner.projection a := Some a.(ArenaL.hp);
  SubPointer.Runner.injection a hp' := Some (ArenaL.mk hp' a.(ArenaL.mem));
|}.

Lemma arena_hp_runner_valid :
  SubPointer.Runner.Valid.t arena_hp_runner.
Proof. constructor; intro a; cbn; reflexivity. Qed.

Definition arena_mem_runner :
    SubPointer.Runner.t ArenaL.t
      (Pointer.Index.StructRecord "encore_vm::arena::Arena" "mem") := {|
  SubPointer.Runner.Sub_A := '&mut (list EncoreValue);
  SubPointer.Runner.H_Sub_A := Ref.IsLink;
  SubPointer.Runner.projection a := Some a.(ArenaL.mem);
  SubPointer.Runner.injection a mem' := Some (ArenaL.mk a.(ArenaL.hp) mem');
|}.

Lemma arena_mem_runner_valid :
  SubPointer.Runner.Valid.t arena_mem_runner.
Proof. constructor; intro a; cbn; reflexivity. Qed.

(* ----------------------------------------------------------------- *)
(** ** Run.Trait instances *)

(** [hp]: read self, get sub-pointer "hp", read. *)
Global Instance run_hp :
  forall (self : '& ArenaL.t),
  Run.Trait arena.Impl_encore_vm_arena_Arena.hp [] [] [φ self] usize.
Proof.
  intro self. constructor.
  unfold arena.Impl_encore_vm_arena_Arena.hp. cbn.
  unshelve eapply Run.CallPrimitiveStateAlloc.
  - exact (@Ref.of_ty_ref _ (@OfTy.of_ty ArenaL.t ArenaIsLink)).
  - exact self.
  - cbn; reflexivity.
  - cbn; intro ref_self.
    eapply Run.CallPrimitiveStateRead; intro self_val.
    eapply Run.Rewrite.
    + rewrite Ref.deref_eq; reflexivity.
    + simpl LowM.let_.
      unshelve eapply Run.CallPrimitiveGetSubPointer.
      * exact arena_hp_runner.
      * exact arena_hp_runner_valid.
      * cbn; intro hp_ref.
        eapply Run.CallPrimitiveStateRead; intro hp_val.
        eapply Run.PureSuccess; reflexivity.
Qed.

(** [new]: construct an Arena from a [&mut [Value]]. *)
Global Instance run_new :
  forall (mem : '&mut (list EncoreValue)),
  Run.Trait arena.Impl_encore_vm_arena_Arena.new [] [] [φ mem] ArenaL.t.
Proof.
  intro mem. constructor.
  unfold arena.Impl_encore_vm_arena_Arena.new. cbn.
  unshelve eapply Run.CallPrimitiveStateAlloc.
  - exact (Ref.of_ty_mut_ref _
      (Slice.of_ty (@OfTy.Make (Ty.path "encore_vm::value::Value")
        EncoreValue EncoreValueLink (eq_sym EncoreValueLink_Φ)))).
  - exact mem.
  - cbn; reflexivity.
  - cbn; intro ref_mem.
    eapply Run.CallPrimitiveStateRead; intro mem_val.
    eapply Run.Rewrite.
    + rewrite Ref.deref_eq; reflexivity.
    + simpl LowM.let_.
      change (borrow Pointer.Kind.MutRef
        (Value.Pointer (Ref.to_pointer (Ref.cast_to Pointer.Kind.Raw mem_val))))
        with (borrow Pointer.Kind.MutRef (φ (Ref.cast_to Pointer.Kind.Raw mem_val))).
      eapply Run.Rewrite.
      * rewrite Ref.borrow_eq; reflexivity.
      * simpl LowM.let_.
        eapply (Run.PureSuccess _ _ _
          (ArenaL.mk (Integer.Build_t _ 0)
            (Ref.cast_to Pointer.Kind.MutRef
              (Ref.cast_to Pointer.Kind.Raw mem_val))));
          cbn; reflexivity.
Qed.

(* ----------------------------------------------------------------- *)
(** ** Auxiliary Run.Trait instances for called functions *)

Require Import encore_vm.value.

(** [HeapAddress::offset]: self.0 as usize + off *)
Global Instance run_heap_address_offset :
  forall (ha : HeapAddressL.t) (off : usize),
  Run.Trait value.Impl_encore_vm_value_HeapAddress.offset [] [] [φ ha; φ off] usize.
Proof.
  intros ha off. constructor.
  unfold value.Impl_encore_vm_value_HeapAddress.offset. cbn.
  unshelve eapply Run.CallPrimitiveStateAlloc.
  - exact (@OfTy.of_ty HeapAddressL.t HeapAddressIsLink).
  - exact ha.
  - cbn; reflexivity.
  - cbn; intro ref_ha.
    unshelve eapply Run.CallPrimitiveStateAlloc.
    + exact (@OfTy.of_ty usize Integer.IsLink).
    + exact off.
    + cbn; reflexivity.
    + cbn; intro ref_off.
      change (deref (Value.Pointer (Ref.to_pointer ref_ha)))
        with (deref (φ ref_ha)).
      eapply Run.Rewrite.
      * rewrite Ref.deref_eq; reflexivity.
      * simpl LowM.let_.
        unshelve eapply Run.CallPrimitiveGetSubPointer.
        -- exact ha_field0_runner.
        -- exact ha_field0_runner_valid.
        -- cbn; intro raw_ref.
           eapply Run.CallPrimitiveStateRead; intro raw_val.
           eapply Run.CallPrimitiveStateRead; intro off_val.
           replace (cast (Ty.path "usize") (Integer.IsLink.(φ) raw_val))
             with (@φ _ (@Integer.IsLink IntegerKind.Usize)
                     (cast_integer IntegerKind.Usize raw_val));
             [| symmetry; exact (cast_integer_eq IntegerKind.U16
                  IntegerKind.Usize raw_val)].
           unfold BinOp.Wrap.add, BinOp.Wrap.make_arithmetic.
           unshelve eapply Run.CallClosure;
             [exact (@OfTy.of_ty usize Integer.IsLink) | | ].
           ++ cbn.
              eapply (Run.PureSuccess _ _ _
                (Integer.Build_t IntegerKind.Usize
                  ((raw_val.(Integer.value) mod 2 ^ 64 +
                    off_val.(Integer.value)) mod 2 ^ 64)));
                cbn; reflexivity.
           ++ cbn. intro v.
              eapply Run.PureSuccess. reflexivity.
Qed.

(** [slice::get_unchecked]: axiomatized (stdlib intrinsic).
    We axiomatize both the function body and its IsAssociatedFunction instance. *)
Parameter slice_get_unchecked_Value : PolymorphicFunction.t.
Axiom slice_get_unchecked_Value_IsAssociated :
  M.IsAssociatedFunction.C
    (Ty.apply (Ty.path "slice") [] [Ty.path "encore_vm::value::Value"])
    "get_unchecked"
    slice_get_unchecked_Value.

Global Instance run_slice_get_unchecked :
  forall (slice_ref : '& (list EncoreValue)) (idx : usize),
  Run.Trait slice_get_unchecked_Value [] [Ty.path "usize"]
    [φ slice_ref; φ idx]
    ('& EncoreValue).
Admitted.

(* ----------------------------------------------------------------- *)
(** ** Run.Trait instances for Arena methods (continued) *)

(** [heap_read]: index into the arena's memory. *)
Global Instance run_heap_read :
  forall (self : '& ArenaL.t) (addr : HeapAddressL.t) (off : usize),
  Run.Trait arena.Impl_encore_vm_arena_Arena.heap_read
    [] [] [φ self; φ addr; φ off] EncoreValue.
Proof.
  intros self addr off. constructor.
  unfold arena.Impl_encore_vm_arena_Arena.heap_read. cbn.
  (* Alloc self, addr, off *)
  unshelve eapply Run.CallPrimitiveStateAlloc.
  - exact (@Ref.of_ty_ref _ (@OfTy.of_ty ArenaL.t ArenaIsLink)).
  - exact self.
  - cbn; reflexivity.
  - cbn; intro ref_self.
    unshelve eapply Run.CallPrimitiveStateAlloc.
    + exact (@OfTy.of_ty HeapAddressL.t HeapAddressIsLink).
    + exact addr.
    + cbn; reflexivity.
    + cbn; intro ref_addr.
      unshelve eapply Run.CallPrimitiveStateAlloc.
      * exact (@OfTy.of_ty usize Integer.IsLink).
      * exact off.
      * cbn; reflexivity.
      * cbn; intro ref_off.
        (* Resolve slice::get_unchecked *)
        unshelve eapply Run.CallPrimitiveGetAssociatedFunction.
        -- exact slice_get_unchecked_Value.
        -- exact slice_get_unchecked_Value_IsAssociated.
        -- (* Read self, deref, sub-pointer "mem", read mem, deref mem, borrow Ref *)
           eapply Run.CallPrimitiveStateRead; intro self_val.
           eapply Run.Rewrite.
           ++ rewrite Ref.deref_eq; reflexivity.
           ++ simpl LowM.let_.
              unshelve eapply Run.CallPrimitiveGetSubPointer.
              ** exact arena_mem_runner.
              ** exact arena_mem_runner_valid.
              ** cbn; intro mem_ref.
                 eapply Run.CallPrimitiveStateRead; intro mem_val.
                 eapply Run.Rewrite.
                 --- rewrite Ref.deref_eq; reflexivity.
                 --- simpl LowM.let_.
                     change (borrow Pointer.Kind.Ref
                       (Value.Pointer (Ref.to_pointer (Ref.cast_to Pointer.Kind.Raw mem_val))))
                       with (borrow Pointer.Kind.Ref (φ (Ref.cast_to Pointer.Kind.Raw mem_val))).
                     eapply Run.Rewrite.
                     +++ rewrite Ref.borrow_eq; reflexivity.
                     +++ simpl LowM.let_.
                         (* Resolve HeapAddress::offset *)
                         unshelve eapply Run.CallPrimitiveGetAssociatedFunction.
                         *** exact value.Impl_encore_vm_value_HeapAddress.offset.
                         *** exact value.Impl_encore_vm_value_HeapAddress.AssociatedFunction_offset.
                         *** (* Read addr and off, call offset *)
                             eapply Run.CallPrimitiveStateRead; intro addr_val.
                             eapply Run.CallPrimitiveStateRead; intro off_val.
                             unshelve eapply Run.CallClosure.
                             ---- exact (@OfTy.of_ty usize Integer.IsLink).
                             ---- cbn. exact (@Run.run_f _ _ _ _ _ _ (run_heap_address_offset addr_val off_val)).
                             ---- cbn; intro idx.
                                  (* Call slice::get_unchecked *)
                                  unshelve eapply Run.CallClosure.
                                  ++++ exact (@Ref.of_ty_ref _ (@OfTy.Make (Ty.path "encore_vm::value::Value")
                                         EncoreValue EncoreValueLink (eq_sym EncoreValueLink_Φ))).
                                  ++++ cbn. exact (@Run.run_f _ _ _ _ _ _
                                         (run_slice_get_unchecked
                                           (Ref.cast_to Pointer.Kind.Ref (Ref.cast_to Pointer.Kind.Raw mem_val))
                                           idx)).
                                  ++++ cbn; intro val_ref.
                                       (* Deref the & Value, read result *)
                                       change (deref (Value.Pointer (Ref.to_pointer val_ref)))
                                         with (deref (φ val_ref)).
                                       eapply Run.Rewrite.
                                       **** rewrite Ref.deref_eq; reflexivity.
                                       **** simpl LowM.let_.
                                            eapply Run.CallPrimitiveStateRead; intro result.
                                            eapply Run.PureSuccess; reflexivity.
Qed.

(** [heap_write]: write a value into the arena's memory. *)
Global Instance run_heap_write :
  forall (self : '&mut ArenaL.t) (addr : HeapAddressL.t) (off : usize) (val : EncoreValue),
  Run.Trait arena.Impl_encore_vm_arena_Arena.heap_write
    [] [] [φ self; φ addr; φ off; φ val] unit.
Admitted.
