use crate::ir::ds;
use crate::pass::{cps_optimize, cps_transform, emit::Emitter, resolver};

pub fn compile_module(module: ds::Module) -> Vec<u8> {
    let cps_module = cps_transform::transform_module(module);
    let cps_module = cps_optimize::optimize_module(cps_module);
    let asm_module = resolver::resolve_module(&cps_module);
    Emitter::emit_module(&asm_module)
}
