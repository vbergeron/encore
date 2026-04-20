pub mod lexer;
pub mod parser;

pub use encore_compiler::ir::ds;
pub use encore_compiler::ir::prim;

use encore_compiler::frontend::{Frontend, ParseError, ParseOutput};

pub struct FlecheFrontend;

impl Frontend for FlecheFrontend {
    fn parse(&self, input: &str) -> Result<ParseOutput, ParseError> {
        let mut parser = parser::Parser::new(input);
        let module = parser.parse_module()?;
        let ctor_names = parser.ctor_names();
        Ok(ParseOutput { module, ctor_names })
    }
}

pub fn parse(input: &str) -> ds::Module {
    FlecheFrontend.parse(input).unwrap().module
}

pub fn parse_with_metadata(input: &str) -> (ds::Module, Vec<(u8, String)>) {
    let output = FlecheFrontend.parse(input).unwrap();
    (output.module, output.ctor_names)
}
