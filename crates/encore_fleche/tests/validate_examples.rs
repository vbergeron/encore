//! Every example program must pass the VM's load-time validation, with and
//! without the optimizer.

use std::path::Path;

use encore_compiler::pass::cps_optimize::OptimizeConfig;
use encore_compiler::pipeline;
use encore_vm::program::Program;

#[test]
fn test_examples_validate() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut seen = 0;
    for entry in std::fs::read_dir(&examples).unwrap() {
        let dir = entry.unwrap().path();
        let name = dir.file_name().unwrap().to_str().unwrap().to_string();
        let src_file = dir.join(format!("{name}.fleche"));
        let Ok(src) = std::fs::read_to_string(&src_file) else { continue };
        for config in [None, Some(OptimizeConfig::default())] {
            let binary = pipeline::compile_module(encore_fleche::parse(&src), config, None).unwrap();
            let prog = Program::parse(&binary).unwrap();
            if let Err(e) = prog.validate() {
                panic!("{}: {e}", src_file.display());
            }
        }
        seen += 1;
    }
    assert!(seen > 0, "no examples found in {}", examples.display());
}
