//! Every IR text format must round-trip: for each example and each stage,
//! print → parse → print is stable, and resuming the pipeline from the
//! parsed text produces the same bytecode as compiling straight through.

use std::path::{Path, PathBuf};

use encore_compiler::frontend::Frontend;
use encore_compiler::ir::{asm, cps, ds, dsi};
use encore_compiler::pass::cps_optimize::OptimizeConfig;
use encore_compiler::pipeline::{self, Ir, IrKind, Stage};
use encore_fleche::FlecheFrontend;
use encore_scheme::SchemeFrontend;

/// The frontends and passes recurse deeply; debug builds need more than the
/// 2 MiB default test-thread stack on the larger examples.
fn with_big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e));
}

fn sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let dir = entry.unwrap().path();
        if !dir.is_dir() { continue; }
        for file in std::fs::read_dir(&dir).unwrap() {
            let p = file.unwrap().path();
            if matches!(p.extension().and_then(|e| e.to_str()), Some("fleche" | "scm")) {
                out.push(p);
            }
        }
    }
    out.sort();
    assert!(!out.is_empty());
    out
}

fn parse_source(path: &Path) -> ds::Module {
    let src = std::fs::read_to_string(path).unwrap();
    let out = match path.extension().unwrap().to_str().unwrap() {
        "fleche" => FlecheFrontend.parse(&src),
        _ => SchemeFrontend.parse(&src),
    };
    out.unwrap_or_else(|e| panic!("{}: {e}", path.display())).module
}

#[test]
fn every_stage_round_trips_through_text() {
    with_big_stack(every_stage_round_trips_through_text_impl);
}

fn every_stage_round_trips_through_text_impl() {
    for path in sources() {
        let module = parse_source(&path);
        let reference = pipeline::compile_module(module.clone(), Some(OptimizeConfig::default()), None).unwrap();
        for stage in Stage::ALL {
            let ctx = format!("{} at {stage}", path.display());
            let ir = pipeline::run_stages(Ir::Ds(module.clone()), Stage::Parse, stage, Some(OptimizeConfig::default())).unwrap();
            let text = ir.to_text();
            let reparsed = Ir::from_text(stage.ir(), &text).unwrap_or_else(|e| panic!("{ctx}: {e}\n{text}"));
            assert_eq!(reparsed.to_text(), text, "{ctx}: text not stable");
            let binary = pipeline::compile_from(reparsed, stage, Some(OptimizeConfig::default()), None).unwrap();
            assert_eq!(binary, reference, "{ctx}: bytecode differs");
        }
    }
}

#[test]
fn structural_equality_for_cps_and_asm() {
    with_big_stack(structural_equality_for_cps_and_asm_impl);
}

fn structural_equality_for_cps_and_asm_impl() {
    for path in sources() {
        let module = parse_source(&path);
        let Ir::Cps(c) = pipeline::run_stages(Ir::Ds(module.clone()), Stage::Parse, Stage::CpsOptimize, Some(OptimizeConfig::default())).unwrap() else { unreachable!() };
        let c2 = cps::Module::from_text(&c.to_text()).unwrap();
        for (a, b) in c.defines.iter().zip(&c2.defines) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.body, b.body);
        }
        let Ir::Asm(m) = pipeline::run_stages(Ir::Ds(module), Stage::Parse, Stage::AsmPeephole, Some(OptimizeConfig::default())).unwrap() else { unreachable!() };
        let m2 = asm::Module::from_text(&m.to_text()).unwrap();
        for (a, b) in m.defines.iter().zip(&m2.defines) {
            assert_eq!(a.global, b.global);
            assert_eq!(a.body, b.body);
        }
    }
}

#[test]
fn hand_written_ir_compiles() {
    let src = r#"
        ; comments and free layout are fine
        (define id (lambda (x) x))
        (define main (app id (bytes "hi\x00")))
    "#;
    let m = ds::Module::from_text(src).unwrap();
    assert_eq!(m.defines.len(), 2);
    pipeline::compile_from(Ir::Ds(m), Stage::Parse, None, None).unwrap();

    let dsi = dsi::Module::from_text("(define main (let 1 (prim int.add (var 0) 41)))").unwrap();
    pipeline::compile_from(Ir::Dsi(dsi), Stage::DsiResolve, None, None).unwrap();
}

#[test]
fn errors_report_line_and_form() {
    let err = ds::Module::from_text("(define a\n  (let x 1))").err().expect("parse should fail");
    assert_eq!(err.line, 2);
    assert!(err.message.contains("`let` expects 3"), "{err}");

    let err = cps::Module::from_text("(define a (fin 3))").err().expect("parse should fail");
    assert!(err.message.contains("expected a symbol"), "{err}");

    let err = asm::Module::from_text("(define 0 (fin r256))").err().expect("parse should fail");
    assert!(err.message.contains("expected a register"), "{err}");

    let err = ds::Module::from_text("(define a (prim int.nope 1))").err().expect("parse should fail");
    assert!(err.message.contains("unknown primitive"), "{err}");

    let err = ds::Module::from_text("(define a (ctor 300))").err().expect("parse should fail");
    assert!(err.message.contains("tag out of range"), "{err}");
}

#[test]
fn stage_names_parse() {
    for stage in Stage::ALL {
        assert_eq!(stage.name().parse::<Stage>().unwrap(), stage);
    }
    assert_eq!(Stage::CpsOptimize.ir(), IrKind::Cps);
    assert!("cps".parse::<Stage>().is_err());
}

#[test]
fn wrong_ir_for_stage_is_an_error() {
    let m = ds::Module::from_text("(define main 1)").unwrap();
    assert!(pipeline::run_stages(Ir::Ds(m), Stage::CpsTransform, Stage::AsmPeephole, None).is_err());
}

