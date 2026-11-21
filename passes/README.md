# passes: Encore compiler passes in Rocq

A second implementation of the compiler pipeline, written in Rocq and
extracted to OCaml. It reads and writes the same IR text as
`encore compile ... --emit/--from` (see [`IR.md`](../IR.md)), so a pass written
here can stand in for its Rust counterpart in the middle of a Rust
compilation.

For now this is a skeleton. The IRs, their text forms and the pipeline are
done. The passes are stubs.

```bash
dune build && dune test    # from the repository root: Rocq 9.1, dune >= 3.21

# Round trip: decode into the Rocq types, re-encode
dune exec encore-passes -- prog.cps --from cps-transform

# Run the Rocq passes after cps-transform up to cps-optimize, then let Rust finish
encore compile fleche prog.fleche --emit cps-transform > prog.cps
dune exec encore-passes -- prog.cps --from cps-transform --emit cps-optimize -o prog.opt
encore compile ir prog.opt --from cps-optimize --out out
```

`--from STAGE` means "this text is the output of STAGE". The tool runs every
pass after it, up to and including `--emit`. `--emit` defaults to `--from`,
which gives a plain round trip. With no input file, or `-`, it reads standard
input.

## Layout

| Path | Language | Contents |
|------|----------|----------|
| `theories/Result.v` | Rocq | Error monad, `mapM` |
| `theories/Sexp.v` | Rocq | The `sexp` tree and decoding helpers (range checks, arity errors) |
| `theories/ir/Prim.v` | Rocq | Primitive operations |
| `theories/ir/{Ds,Dsi,Cps,Asm}.v` | Rocq | Each IR as a Rocq type, with `encode : module -> list sexp` and `decode : list sexp -> result module` |
| `theories/pass/*.v` | Rocq | One file per pass, `run : <IR>.module -> result <IR>.module` |
| `theories/Pipeline.v` | Rocq | Stages, the `ir` sum type, `compile from to` |
| `theories/semantics/Runtime.v` | Rocq | What every machine-level semantics shares, parameterized by the type of code addresses: words, heap, allocation, the collector specification (assumed correct), primitives, errors, and observations (`obs`, `load_obs`, `refines`) |
| `theories/semantics/Machine.v` | Rocq | Semantics of the bytecode VM, transcribed from `crates/encore_vm/src/vm.rs`: instructions, step relation with traps, `load` and calls, and `behaves`, the observed behaviour of a program |
| `theories/semantics/Correctness.v` | Rocq | `correct`: a transformation preserves behaviour relative to a source and a target semantics; `preserves` for Encore's load observations; identity and composition lemmas |
| `extraction/Extract.v` | Rocq | Extracts `Pipeline` to `encore_passes.ml`, an OCaml library |
| `bin/sexp_text.ml` | OCaml | Text to `sexp` and back. Matches the Rust reader and printer byte for byte |
| `bin/main.ml` | OCaml | The `encore-passes` command line |
| `test/passes.t/` | cram | Round trips every stage of a Rust-compiled example, plus the error paths |

OCaml only handles bytes ↔ `sexp` and the command line. Turning `sexp` into
typed IR, the passes, and turning IR back into `sexp` all happen in Rocq.

Extraction maps `nat` and `Z` to OCaml `int` and `string` to the native OCaml
string (`ExtrOcamlNatInt`, `ExtrOcamlZInt`, `ExtrOcamlNativeString`). Every
number in IR text fits: the reader rejects integers too large for an `int`, and
the decoders check the IR's own bounds (`u8` tags, `i32` literals, ...).

## Passes

| Stage | Pass file | Status |
|-------|-----------|--------|
| `ds-uncurry` | `pass/DsUncurry.v` | not implemented (error) |
| `dsi-resolve` | `pass/DsiResolve.v` | not implemented (error) |
| `cps-transform` | `pass/CpsTransform.v` | not implemented (error) |
| `cps-optimize` | `pass/CpsOptimize.v` | identity, like Rust's `--cps-optimize off` |
| `asm-resolve` | `pass/AsmResolve.v` | not implemented (error) |
| `asm-peephole` | `pass/AsmPeephole.v` | identity |

To implement a pass, replace the body of `run` in its file. Nothing else
needs to change: `Pipeline.step` already sends it the right IR.

## Differences from the Rust reader

Decode errors coming from Rocq have no line number, because `sexp` does not
carry source positions. Lexical errors from the OCaml reader do have one.
