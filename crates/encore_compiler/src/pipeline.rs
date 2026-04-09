use crate::ir::ds;
use crate::pass::{cps_optimize::{self, OptimizeConfig}, cps_transform, emit::Emitter, resolver};

pub fn compile_module(module: ds::Module) -> Vec<u8> {
    compile_module_with_config(module, OptimizeConfig::default())
}

pub fn compile_module_with_config(module: ds::Module, config: OptimizeConfig) -> Vec<u8> {
    let cps_module = cps_transform::transform_module(module);
    let cps_module = cps_optimize::optimize_module(cps_module, config);
    let asm_module = resolver::resolve_module(&cps_module);
    Emitter::emit_module(&asm_module)
}
