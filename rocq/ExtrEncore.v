(** * Extraction of Rocq programs to Encore

    Importing this module sets the extraction language to Scheme and maps
    [nat] and its operations onto Encore VM integers and primitives. The
    extracted [.scm] file is then compiled with [encore compile scheme].

    Every directive below replaces a proven Gallina definition with an
    unproven piece of Scheme. They are the trust assumptions of any program
    extracted through this module, and are listed with their preconditions
    in the "Trust assumptions" section of SCHEME.md. The main one:

    - [nat] becomes a 24-bit VM integer. [nat] is unbounded; the VM is not.
      Every [nat] the program builds must stay below 2^23 (8_388_608). An
      operation whose result leaves that range traps with [IntOverflow]
      rather than returning a wrong answer, so the precondition is checked
      at run time, but the extracted program is only correct if it holds.

    ** Inductive types

    [bool], [list] and [prod] keep their constructor representation, with
    the constructor names pinned to the ones Encore pre-registers with
    fixed tags:

    | Gallina         | Scheme  | Tag |
    |-----------------|---------|-----|
    | [false]         | [False] | 0   |
    | [true]          | [True]  | 1   |
    | [nil]           | [Nil]   | 2   |
    | [cons]          | [Cons]  | 3   |
    | [pair]          | [Pair]  | 4   |

    Rocq's default naming (the Gallina name, capitalised) gives the same
    names, but only until another extracted inductive has a clashing
    constructor: Rocq then renames one of them. A [nat] literal above 5000,
    for instance, pulls in [Decimal.uint], whose [Nil] would turn [list]'s
    into [Nil1]. The frontend would give it a fresh tag, not the one the
    host expects for lists; for [bool], the tags VM comparisons return
    would no longer match the Gallina [bool] at all.
    The names are checked against the registry by
    [crates/encore_scheme/tests/rocq_extraction.rs].

    VM comparisons return tags 0/1, so the [bool] they produce is the
    [bool] Gallina matches on. Other inductives get tags from 5 upward in
    order of first use.

    [ascii] and [string] are also left at their default extraction on
    purpose: the Scheme frontend folds every closed [String]/[Ascii]
    literal chain into a native byte string at compile time. See
    [ExtrEncoreBytes] for how to use them.

    ** Why every [nat] directive is written twice

    The directives name both [Init.Nat.*] and [PeanoNat.Nat.*]. The
    notations [+], [*], [-] unfold to [Init.Nat.*]. But importing [Arith]
    (or anything that exports [PeanoNat]) makes [Nat.eqb], [Nat.div], ...
    and the [=?], [<=?], [<?], [/], [mod] notations resolve to
    [PeanoNat.Nat.*], whose constants are aliases that extraction treats as
    distinct. With only one set, the directives silently do not apply to
    the other: [=?] then extracts to the recursive Gallina [eqb], which
    still computes the right answer through the [nat] eliminator above, but
    in time linear in its arguments instead of one VM instruction. *)

From Stdlib Require Extraction.
From Stdlib Require Arith.PeanoNat.

Extraction Language Scheme.

(** Constructor names pinned to Encore's pre-registered ones (see above). *)
Extract Inductive bool => "bool" ["True" "False"].
Extract Inductive list => "list" ["Nil" "Cons"].
Extract Inductive prod => "prod" ["Pair"].

(** [nat] is a VM integer. [O] is [0], [S] is [+ 1], and a match on [nat]
    is an eliminator that tests for zero. *)
Extract Inductive nat => "integer"
  ["0" "(lambda (x) (+ x 1))"]
  "(lambdas (fO fS n) (if (= n 0) (fO 0) (fS (- n 1))))".

(** ** [Init.Nat] *)

Extract Constant Init.Nat.add => "(lambda (a) (lambda (b) (+ a b)))".
Extract Constant Init.Nat.mul => "(lambda (a) (lambda (b) (* a b)))".
Extract Constant Init.Nat.sub => "(lambda (a) (lambda (b) (int-sub-sat a b)))".
Extract Constant Init.Nat.pred => "(lambda (a) (int-sub-sat a 1))".
Extract Constant Init.Nat.min => "(lambda (a) (lambda (b) (if (<= a b) a b)))".
Extract Constant Init.Nat.max => "(lambda (a) (lambda (b) (if (<= a b) b a)))".
Extract Constant Init.Nat.eqb => "(lambda (a) (lambda (b) (= a b)))".
Extract Constant Init.Nat.leb => "(lambda (a) (lambda (b) (<= a b)))".
Extract Constant Init.Nat.ltb => "(lambda (a) (lambda (b) (< a b)))".
Extract Constant Init.Nat.even => "(lambda (a) (= (int-and a 1) 0))".
Extract Constant Init.Nat.odd => "(lambda (a) (= (int-and a 1) 1))".
Extract Constant Init.Nat.div => "(lambda (a) (lambda (b) (int-div a b)))".
Extract Constant Init.Nat.modulo => "(lambda (a) (lambda (b) (int-mod a b)))".
Extract Constant Init.Nat.div2 => "(lambda (a) (int-shr a 1))".
Extract Constant Init.Nat.land => "(lambda (a) (lambda (b) (int-and a b)))".
Extract Constant Init.Nat.lor => "(lambda (a) (lambda (b) (int-or a b)))".
Extract Constant Init.Nat.lxor => "(lambda (a) (lambda (b) (int-xor a b)))".
Extract Constant Init.Nat.shiftl => "(lambda (a) (lambda (b) (int-shl a b)))".
Extract Constant Init.Nat.shiftr => "(lambda (a) (lambda (b) (int-shr a b)))".

(** ** [PeanoNat.Nat]: the same, for the aliases (see above) *)

Extract Constant PeanoNat.Nat.add => "(lambda (a) (lambda (b) (+ a b)))".
Extract Constant PeanoNat.Nat.mul => "(lambda (a) (lambda (b) (* a b)))".
Extract Constant PeanoNat.Nat.sub => "(lambda (a) (lambda (b) (int-sub-sat a b)))".
Extract Constant PeanoNat.Nat.pred => "(lambda (a) (int-sub-sat a 1))".
Extract Constant PeanoNat.Nat.min => "(lambda (a) (lambda (b) (if (<= a b) a b)))".
Extract Constant PeanoNat.Nat.max => "(lambda (a) (lambda (b) (if (<= a b) b a)))".
Extract Constant PeanoNat.Nat.eqb => "(lambda (a) (lambda (b) (= a b)))".
Extract Constant PeanoNat.Nat.leb => "(lambda (a) (lambda (b) (<= a b)))".
Extract Constant PeanoNat.Nat.ltb => "(lambda (a) (lambda (b) (< a b)))".
Extract Constant PeanoNat.Nat.even => "(lambda (a) (= (int-and a 1) 0))".
Extract Constant PeanoNat.Nat.odd => "(lambda (a) (= (int-and a 1) 1))".
Extract Constant PeanoNat.Nat.div => "(lambda (a) (lambda (b) (int-div a b)))".
Extract Constant PeanoNat.Nat.modulo => "(lambda (a) (lambda (b) (int-mod a b)))".
Extract Constant PeanoNat.Nat.div2 => "(lambda (a) (int-shr a 1))".
Extract Constant PeanoNat.Nat.land => "(lambda (a) (lambda (b) (int-and a b)))".
Extract Constant PeanoNat.Nat.lor => "(lambda (a) (lambda (b) (int-or a b)))".
Extract Constant PeanoNat.Nat.lxor => "(lambda (a) (lambda (b) (int-xor a b)))".
Extract Constant PeanoNat.Nat.shiftl => "(lambda (a) (lambda (b) (int-shl a b)))".
Extract Constant PeanoNat.Nat.shiftr => "(lambda (a) (lambda (b) (int-shr a b)))".
