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
