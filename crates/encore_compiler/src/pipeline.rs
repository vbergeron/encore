use crate::ir::ds;
use crate::pass::{asm_emit::Emitter, asm_resolve, cps_optimize::{self, OptimizeConfig}, cps_transform, dsi_resolve};

pub fn compile_module(module: ds::Module) -> Vec<u8> {
    compile_module_with_config(module, OptimizeConfig::default())
}

pub fn compile_module_with_config(module: ds::Module, config: OptimizeConfig) -> Vec<u8> {
    let dsi_module = dsi_resolve::resolve_module(module);
    let cps_module = cps_transform::transform_module(dsi_module);
    let cps_module = cps_optimize::optimize_module(cps_module, config);
    let asm_module = asm_resolve::resolve_module(&cps_module);
    Emitter::emit_module(&asm_module)
}

pub fn compile_module_unoptimized(module: ds::Module) -> Vec<u8> {
    let dsi_module = dsi_resolve::resolve_module(module);
    let cps_module = cps_transform::transform_module(dsi_module);
    let asm_module = asm_resolve::resolve_module(&cps_module);
    Emitter::emit_module(&asm_module)
}
