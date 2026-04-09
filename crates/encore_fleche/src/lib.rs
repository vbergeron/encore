pub mod lexer;
pub mod parser;

pub use encore_compiler::ir::ds;
pub use encore_compiler::ir::prim;

pub fn parse(input: &str) -> ds::Module {
    let mut parser = parser::Parser::new(input);
    parser.parse_module()
}
