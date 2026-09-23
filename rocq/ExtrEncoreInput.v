(** * Reading host data through an extern

    A program that consumes a buffer owned by the host (a received frame,
    a flash page, ...) should not have the host copy it into the VM heap
    as a [list nat]: it costs a cons cell per byte, and a list built from
    Rust is not a GC root while it is being built. Instead the Gallina
    reads one byte at a time through an Encore extern, the way firmware
    reads a receive buffer.

    The idiom is a [Parameter] realised by [(extern (slot N) args...)]:

    {[
      Parameter input_byte : nat -> nat.
      Extract Constant input_byte => "(extern (slot 0) i)".
    ]}

    The Scheme frontend turns [(extern (slot N) a b ...)] into a curried
    function of [a b ...] that packs its arguments into one constructor
    and calls the host function registered in slot [N]. With no argument
    names, the constant is the raw extern value. Pick a slot per extern and
    register the host function in the same slot on the Rust side.

    This module provides that idiom for slot 0. Importing it reserves slot
    0 for [input_byte]; a program that needs other externs declares them
    the same way in its own module, in other slots.

    Trust assumption (also in SCHEME.md): [input_byte] is an axiom,
    realised by the host function in slot 0. The host must return a value
    in [0, 255] for every index the program reads, and the same value every
    time it is asked for the same index. Nothing is proven about it: state
    proofs over the lists [read_bytes] returns, not over the host buffer. *)

From Stdlib Require Extraction.
From Stdlib Require Import List.
Import ListNotations.
From Encore.Extraction Require Import ExtrEncore.

Parameter input_byte : nat -> nat.

Extract Constant input_byte => "(extern (slot 0) i)".

(** [read_bytes pos n] is the list of the [n] input bytes from [pos]. *)
Fixpoint read_bytes (pos n : nat) : list nat :=
  match n with
  | O => []
  | S n' => input_byte pos :: read_bytes (S pos) n'
  end.
