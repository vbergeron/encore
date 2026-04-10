pub mod lexer;
pub mod parser;

pub use encore_compiler::ir::ds;
pub use encore_compiler::ir::prim;

pub fn parse(input: &str) -> ds::Module {
    let mut parser = parser::Parser::new(input);
    parser.parse_module()
}

pub fn parse_with_metadata(input: &str) -> (ds::Module, Vec<(u8, String)>) {
    let mut parser = parser::Parser::new(input);
    let module = parser.parse_module();
    let ctor_names = parser.ctor_names();
    (module, ctor_names)
}
