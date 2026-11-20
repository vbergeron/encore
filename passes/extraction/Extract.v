(** Extraction of the pipeline to OCaml, used by the [encore-passes] driver
    in ../bin. Machine integers replace [nat] and [Z]: every number in the IR
    text fits in an OCaml [int] (the reader rejects anything larger), and
    [string] becomes the native OCaml (byte) string. *)

From Stdlib Require Import Extraction ZArith.
From Stdlib Require Import ExtrOcamlBasic ExtrOcamlNativeString ExtrOcamlNatInt ExtrOcamlZInt.
From EncorePasses Require Import Sexp Pipeline.

(* dune runs this file in the directory the .ml files must land in. *)
Set Warnings "-extraction-default-directory".

Extraction "encore_passes.ml"
  Pipeline.stage_names Pipeline.stage_of_name Pipeline.stage_name Pipeline.compile.
