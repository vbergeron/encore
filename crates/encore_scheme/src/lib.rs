pub mod parser;
pub mod ir;
pub mod desugar;

use encore_compiler::frontend::{Frontend, ParseError, ParseOutput};
use encore_compiler::ir::ds;

pub struct SchemeFrontend;

impl Frontend for SchemeFrontend {
    fn parse(&self, input: &str) -> Result<ParseOutput, ParseError> {
        let sexps = parser::parse(input)
            .map_err(|e| ParseError::from(format!("parse: {e}")))?;
        let scheme_module = desugar::parse_program(&sexps)
            .map_err(|e| ParseError::from(format!("desugar: {e}")))?;
        let (module, ctor_names) = desugar::lower_module(scheme_module);
        Ok(ParseOutput { module, ctor_names })
    }
}

pub fn parse(input: &str) -> ds::Module {
    SchemeFrontend.parse(input).unwrap().module
}

pub fn parse_with_metadata(input: &str) -> (ds::Module, Vec<(u8, String)>) {
    let output = SchemeFrontend.parse(input).unwrap();
    (output.module, output.ctor_names)
}
