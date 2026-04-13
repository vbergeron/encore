pub mod parser;
pub mod ir;
pub mod desugar;

use encore_compiler::ir::ds;

pub fn parse(input: &str) -> ds::Module {
    let sexps = parser::parse(input).unwrap_or_else(|e| {
        panic!("scheme parse error: {e}");
    });
    let scheme_module = desugar::parse_program(&sexps).unwrap_or_else(|e| {
        panic!("scheme desugar error: {e}");
    });
    let (ds_module, _) = desugar::lower_module(scheme_module);
    ds_module
}

pub fn parse_with_metadata(input: &str) -> (ds::Module, Vec<(u8, String)>) {
    let sexps = parser::parse(input).unwrap_or_else(|e| {
        panic!("scheme parse error: {e}");
    });
    let scheme_module = desugar::parse_program(&sexps).unwrap_or_else(|e| {
        panic!("scheme desugar error: {e}");
    });
    desugar::lower_module(scheme_module)
}
