(lang dune 3.21)
(using rocq 0.11)
(name rocq-encore)

(generate_opam_files true)

(source (github vbergeron/encore))
(license Apache-2.0)

(package
 (name rocq-encore)
 (synopsis "Extraction of Rocq programs to the Encore VM")
 (description
  "Extraction directives mapping nat, its operations, byte strings and host externs onto Encore VM values and primitives. Import Encore.Extraction.ExtrEncore before extracting to Scheme, then compile the .scm with `encore compile scheme`.")
 (depends
  (dune (>= 3.21))
  (rocq-core (and (>= 9.1) (< 9.2)))
  (rocq-stdlib (and (>= 9.1) (< 9.2)))))

(package
 (name encore-passes)
 (synopsis "Encore compiler passes written in Rocq")
 (description
  "The Encore IRs (ds, dsi, cps, asm) as Rocq data types, with their S-expression text forms and the compiler passes between them, extracted to OCaml. The encore-passes command reads IR text from one pipeline stage and writes the IR of a later stage.")
 (depends
  (dune (>= 3.21))
  (ocaml (>= 4.14))
  (rocq-core (and (>= 9.1) (< 9.2)))
  (rocq-stdlib (and (>= 9.1) (< 9.2)))))
