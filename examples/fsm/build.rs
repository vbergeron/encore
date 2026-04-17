use std::path::Path;

fn main() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let name = manifest_dir.file_name().unwrap().to_str().unwrap();

    println!("cargo::rustc-link-search={}", manifest_dir.display());
    println!("cargo::rerun-if-changed=memory.x");

    let (module, ctor_names) = {
        #[cfg(feature = "fleche")]
        {
            let src_file = manifest_dir.join(format!("{name}.fleche"));
            println!("cargo::rerun-if-changed={}", src_file.display());
            let src = std::fs::read_to_string(&src_file).expect("read source");
            encore_fleche::parse_with_metadata(&src)
        }
        #[cfg(feature = "scheme")]
        {
            let src_file = manifest_dir.join(format!("{name}.scm"));
            println!("cargo::rerun-if-changed={}", src_file.display());
            let src = std::fs::read_to_string(&src_file).expect("read source");
            encore_scheme::parse_with_metadata(&src)
        }
    };

    encore_compiler::pipeline::compile_to_dir_with_ctors(
        &module,
        Some(encore_compiler::pass::cps_optimize::OptimizeConfig::default()),
        true,
        Path::new(&out_dir),
        &ctor_names,
    )
    .expect("compile");
}
