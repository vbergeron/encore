//! The contract between `rocq/ExtrEncore.v` and this frontend.
//!
//! The `.scm` files below are extracted by `dune build` from the Rocq
//! examples through `Encore.Extraction`, and committed. CI re-extracts them
//! and fails if they differ, so these tests run on what the Rocq side
//! actually produces.

use encore_compiler::frontend::CtorRegistry;
use encore_compiler::pass::cps_optimize::OptimizeConfig;
use encore_compiler::pipeline;
use encore_vm::program::Program;
use encore_vm::value::{GlobalAddress, Value};
use encore_vm::vm::Vm;

/// `ExtrEncore.v` pins these Scheme names for `bool`, `list` and `prod`,
/// and documents their tags. VM comparisons return tags 0/1 as `bool`.
#[test]
fn extrencore_constructor_names_match_registry() {
    let reg = CtorRegistry::new();
    assert_eq!(reg.get("False"), Some((0, 0)));
    assert_eq!(reg.get("True"), Some((1, 0)));
    assert_eq!(reg.get("Nil"), Some((2, 0)));
    assert_eq!(reg.get("Cons"), Some((3, 2)));
    assert_eq!(reg.get("Pair"), Some((4, 2)));
}

/// Compile extracted Scheme with and without the optimizer, and evaluate
/// the integer define named `entry` in both.
///
/// Runs on a thread with a large stack: extracted `nat` literals are
/// successor chains as deep as their value, and the frontend recurses once
/// per level, which overflows the default test stack in debug builds.
fn eval_extracted_int(source: &'static str, entry: &'static str) -> i32 {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || eval_extracted_int_inner(source, entry))
        .unwrap()
        .join()
        .unwrap()
}

fn eval_extracted_int_inner(source: &str, entry: &str) -> i32 {
    let module = encore_scheme::parse(source);
    let idx = module
        .defines
        .iter()
        .position(|d| d.name == entry)
        .unwrap_or_else(|| panic!("no define named {entry}"));
    let results: Vec<i32> = [None, Some(OptimizeConfig::default())]
        .into_iter()
        .map(|config| {
            let binary = pipeline::compile_module(module.clone(), config, None).unwrap();
            let prog = Program::parse(&binary).unwrap();
            let mut mem = [Value::from_u32(0); 8192];
            let mut vm = Vm::init(&mut mem);
            vm.load(&prog).unwrap();
            vm.global_raw(GlobalAddress::new(idx as u16)).int_value().unwrap()
        })
        .collect();
    assert_eq!(results[0], results[1], "{entry}: optimizer changed the result");
    results[0]
}

#[test]
fn extracted_gcd_check() {
    let v = eval_extracted_int(include_str!("../../../examples/gcd/gcd.scm"), "check");
    assert_eq!(v, 6);
}

#[test]
fn extracted_digits_check() {
    // `check` is the length of `render 907` if it equals "n=907".
    let v = eval_extracted_int(include_str!("../../../examples/digits/digits.scm"), "check");
    assert_eq!(v, 5);
}
