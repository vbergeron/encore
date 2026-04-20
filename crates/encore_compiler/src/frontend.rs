use std::collections::BTreeMap;
use std::fmt;
use crate::ir::ds;
use encore_vm::builtins::*;

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<String> for ParseError {
    fn from(message: String) -> Self {
        ParseError { message }
    }
}

impl From<&str> for ParseError {
    fn from(message: &str) -> Self {
        ParseError { message: message.to_string() }
    }
}

pub struct ParseOutput {
    pub module: ds::Module,
    pub ctor_names: Vec<(u8, String)>,
}

pub trait Frontend {
    fn parse(&self, input: &str) -> Result<ParseOutput, ParseError>;
}

struct CtorInfo {
    tag: u8,
    arity: u8,
    type_id: u8,
}

pub struct CtorRegistry {
    ctors: BTreeMap<String, CtorInfo>,
    next_tag: u8,
    next_type_id: u8,
}

impl CtorRegistry {
    pub fn new() -> Self {
        let mut ctors = BTreeMap::new();
        ctors.insert("False".into(), CtorInfo { tag: TAG_FALSE, arity: ARITY_FALSE, type_id: 0 });
        ctors.insert("True".into(), CtorInfo { tag: TAG_TRUE, arity: ARITY_TRUE, type_id: 0 });
        ctors.insert("Nil".into(), CtorInfo { tag: TAG_NIL, arity: ARITY_NIL, type_id: 1 });
        ctors.insert("Cons".into(), CtorInfo { tag: TAG_CONS, arity: ARITY_CONS, type_id: 1 });
        ctors.insert("Pair".into(), CtorInfo { tag: TAG_PAIR, arity: ARITY_PAIR, type_id: 2 });
        Self { ctors, next_tag: FIRST_USER_TAG, next_type_id: 3 }
    }

    pub fn alloc_type_id(&mut self) -> u8 {
        let id = self.next_type_id;
        self.next_type_id += 1;
        id
    }

    pub fn resolve(&mut self, name: &str, arity: u8) -> u8 {
        self.resolve_with_type(name, arity, 0)
    }

    pub fn resolve_with_type(&mut self, name: &str, arity: u8, type_id: u8) -> u8 {
        if let Some(info) = self.ctors.get(name) {
            return info.tag;
        }
        let tag = self.next_tag;
        self.next_tag += 1;
        self.ctors.insert(name.to_string(), CtorInfo { tag, arity, type_id });
        tag
    }

    pub fn get(&self, name: &str) -> Option<(u8, u8)> {
        self.ctors.get(name).map(|info| (info.tag, info.arity))
    }

    pub fn get_with_type(&self, name: &str) -> Option<(u8, u8, u8)> {
        self.ctors.get(name).map(|info| (info.tag, info.arity, info.type_id))
    }

    pub fn ctors_of_type(&self, type_id: u8) -> Vec<(u8, u8)> {
        let mut result: Vec<(u8, u8)> = self.ctors.values()
            .filter(|info| info.type_id == type_id)
            .map(|info| (info.tag, info.arity))
            .collect();
        result.sort_by_key(|&(tag, _)| tag);
        result
    }

    pub fn ctor_names(&self) -> Vec<(u8, String)> {
        self.ctors.iter()
            .map(|(name, info)| (info.tag, name.clone()))
            .collect()
    }
}
