use std::path::Path;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let scheme = manifest_dir.join("scheme/eval.scm");

    println!("cargo::rustc-link-search={}", manifest_dir.display());
    println!("cargo::rerun-if-changed=memory.x");
    println!("cargo::rerun-if-changed={}", scheme.display());

    let src = std::fs::read_to_string(&scheme).expect("read eval.scm");
    let (module, ctor_names) = encore_scheme::parse_with_metadata(&src);
    encore_compiler::pipeline::compile_to_dir_with_ctors(
        &module,
        Some(encore_compiler::pass::cps_optimize::OptimizeConfig::default()),
        true,
        Path::new(&out_dir),
        &ctor_names,
    ).expect("compile");
}
