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
