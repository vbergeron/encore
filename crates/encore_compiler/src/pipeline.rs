use std::path::Path;

use crate::ir::ds;
use crate::pass::{asm_emit::{Emitter, Metadata}, asm_resolve, cps_optimize::{self, OptimizeConfig}, cps_transform, dsi_resolve};

pub fn compile_module(
    module: ds::Module,
    config: Option<OptimizeConfig>,
    metadata: Option<&Metadata>,
) -> Vec<u8> {
    let dsi_module = dsi_resolve::resolve_module(module);
    let cps_module = cps_transform::transform_module(dsi_module);
    let cps_module = match config {
        Some(config) => cps_optimize::optimize_module(cps_module, config),
        None => cps_module,
    };
    let asm_module = asm_resolve::resolve_module(&cps_module);
    Emitter::emit_module(&asm_module, metadata)
}

pub fn compile_to_dir(
    module: &ds::Module,
    config: Option<OptimizeConfig>,
    include_bindings: bool,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    compile_to_dir_with_ctors(module, config, include_bindings, dir, &[])
}

pub fn compile_to_dir_with_ctors(
    module: &ds::Module,
    config: Option<OptimizeConfig>,
    include_bindings: bool,
    dir: &Path,
    ctor_names: &[(u8, String)],
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;

    let global_names: Vec<(u8, String)> = module.defines.iter()
        .enumerate()
        .map(|(i, d)| (i as u8, d.name.clone()))
        .collect();

    let metadata = Metadata {
        ctor_names: ctor_names.to_vec(),
        global_names: global_names.clone(),
    };

    let binary = compile_module(module.clone(), config, Some(&metadata));
    std::fs::write(dir.join("bytecode.bin"), &binary)?;

    if include_bindings {
        let mut s = String::new();
        s.push_str("pub mod funcs {\n");
        for (idx, name) in &global_names {
            s.push_str(&format!(
                "    #[allow(dead_code)]\n    pub const {}: usize = {};\n",
                rust_const_name(name), idx,
            ));
        }
        s.push_str("}\n\n");

        s.push_str("pub mod ctors {\n");
        for (idx, name) in ctor_names {
            s.push_str(&format!(
                "    #[allow(dead_code)]\n    pub const {}: u8 = {};\n",
                rust_const_name(name), idx,
            ));
        }
        s.push_str("}\n");

        std::fs::write(dir.join("bindings.rs"), &s)?;
    }

    Ok(())
}

fn rust_const_name(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}
