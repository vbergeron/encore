(** * Byte strings

    Encore has a native byte-string value; Gallina does not. This module
    declares an abstract type [bytes] with the VM's byte-string primitives
    as axioms, each realised by one primitive at extraction.

    [string] literals reach the VM as byte strings: Rocq extracts a
    [string] literal as a chain of [String]/[Ascii] constructors, and the
    Scheme frontend folds every closed chain into a byte-string literal at
    compile time. [bytes_of_string] is therefore the identity. That only
    holds for literals, which is the reason for the first precondition
    below.

    Trust assumptions (also in SCHEME.md):

    - [string] and [ascii] are for literals only. Apply [bytes_of_string]
      to a [string] literal (or to a constant defined as one), and never
      compute on [string] or [ascii] values: a folded literal is a byte
      string, not a [String] constructor, so Gallina that matches on it
      (including [String.append], [String.length], [String.eqb], ...) is
      miscompiled.
    - [bytes_get b i] requires [i < bytes_len b]. The VM does not check the
      bound and reads past the end.
    - [byte_of_nat n] requires [n <= 255]; otherwise the VM traps with
      [ByteRange].
    - The equations below are all Gallina knows about [bytes]. They are
      axioms, stated so that proofs can use them, and they are what the
      VM primitives are trusted to satisfy. *)

From Stdlib Require Extraction.
From Stdlib Require Import String.
From Encore.Extraction Require Import ExtrEncore.

Parameter bytes : Type.

Parameter bytes_of_string : string -> bytes.
Parameter bytes_len : bytes -> nat.
Parameter bytes_get : bytes -> nat -> nat.
Parameter bytes_concat : bytes -> bytes -> bytes.
Parameter bytes_eqb : bytes -> bytes -> bool.
Parameter byte_of_nat : nat -> bytes.

Axiom bytes_len_of_string :
  forall s, bytes_len (bytes_of_string s) = String.length s.
Axiom bytes_len_concat :
  forall a b, bytes_len (bytes_concat a b) = bytes_len a + bytes_len b.
Axiom bytes_len_byte_of_nat :
  forall n, n <= 255 -> bytes_len (byte_of_nat n) = 1.
Axiom bytes_eqb_spec :
  forall a b, bytes_eqb a b = true <-> a = b.

Extract Constant bytes => "bytes".
Extract Constant bytes_of_string => "(lambda (s) s)".
Extract Constant bytes_len => "(lambda (b) (bytes-len b))".
Extract Constant bytes_get => "(lambda (b) (lambda (i) (bytes-get b i)))".
Extract Constant bytes_concat => "(lambda (a) (lambda (b) (bytes-concat a b)))".
Extract Constant bytes_eqb => "(lambda (a) (lambda (b) (bytes-eq a b)))".
Extract Constant byte_of_nat => "(lambda (n) (int->byte n))".
