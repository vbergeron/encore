pub mod lexer;
pub mod parser;

use crate::ir::ds;

pub fn parse(input: &str) -> ds::Module {
    let mut parser = parser::Parser::new(input);
    parser.parse_module()
}
