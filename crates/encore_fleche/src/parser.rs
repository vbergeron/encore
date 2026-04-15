use std::collections::HashMap;

use crate::ds;
use crate::prim::{PrimOp, IntOp, BytesOp};
use crate::lexer::{Lexer, Token};

struct CtorInfo {
    tag: u8,
    arity: u8,
}

pub struct Parser {
    lexer: Lexer,
    ctors: HashMap<String, CtorInfo>,
    next_tag: u8,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        let mut ctors = HashMap::new();
        ctors.insert("False".into(), CtorInfo { tag: 0, arity: 0 });
        ctors.insert("True".into(), CtorInfo { tag: 1, arity: 0 });
        Self {
            lexer: Lexer::new(input),
            ctors,
            next_tag: 2,
        }
    }

    pub fn parse_module(&mut self) -> ds::Module {
        while *self.lexer.peek() == Token::Data {
            self.parse_data();
        }

        let mut defines = Vec::new();
        while *self.lexer.peek() != Token::Eof {
            defines.push(self.parse_define());
        }

        ds::Module { defines }
    }

    pub fn ctor_names(&self) -> Vec<(u8, String)> {
        self.ctors.iter()
            .map(|(name, info)| (info.tag, name.clone()))
            .collect()
    }

    fn parse_data(&mut self) {
        self.lexer.expect(&Token::Data);

        if *self.lexer.peek() == Token::Pipe {
            self.lexer.next();
        }

        self.parse_variant();

        while *self.lexer.peek() == Token::Pipe {
            self.lexer.next();
            self.parse_variant();
        }
    }

    fn parse_variant(&mut self) {
        let name = match self.lexer.next() {
            Token::UpperIdent(s) => s,
            tok => panic!("expected constructor name, got {tok:?}"),
        };

        let mut arity: u8 = 0;
        if *self.lexer.peek() == Token::LParen {
            self.lexer.next();
            if *self.lexer.peek() != Token::RParen {
                self.expect_lower_ident();
                arity = 1;
                while *self.lexer.peek() == Token::Comma {
                    self.lexer.next();
                    self.expect_lower_ident();
                    arity += 1;
                }
            }
            self.lexer.expect(&Token::RParen);
        }

        if self.ctors.contains_key(&name) {
            return;
        }

        let tag = self.next_tag;
        self.next_tag += 1;
        self.ctors.insert(name, CtorInfo { tag, arity });
    }

    fn expect_lower_ident(&mut self) -> String {
        match self.lexer.next() {
            Token::LowerIdent(s) => s,
            tok => panic!("expected identifier, got {tok:?}"),
        }
    }

    fn parse_define(&mut self) -> ds::Define {
        self.lexer.expect(&Token::Define);
        if *self.lexer.peek() == Token::Extern {
            self.lexer.next();
            let name = self.expect_lower_ident();
            let slot = match self.lexer.next() {
                Token::Number(n) => n as u16,
                tok => panic!("expected extern slot index, got {tok:?}"),
            };
            return ds::Define { name, body: ds::Expr::Extern(slot) };
        }
        let name = self.expect_lower_ident();
        self.lexer.expect(&Token::As);
        let body = self.parse_expr();
        ds::Define { name, body }
    }

    fn parse_expr(&mut self) -> ds::Expr {
        match self.lexer.peek() {
            Token::Let => self.parse_let(),
            Token::Match => self.parse_match(),
            Token::Field => self.parse_field(),
            Token::Builtin => self.parse_builtin(),
            Token::LowerIdent(_) => {
                let name = self.expect_lower_ident();
                if *self.lexer.peek() == Token::Arrow {
                    self.lexer.next();
                    let mut params = vec![name];
                    while let Token::LowerIdent(_) = self.lexer.peek() {
                        let next = self.expect_lower_ident();
                        if *self.lexer.peek() == Token::Arrow {
                            self.lexer.next();
                            params.push(next);
                        } else {
                            let body_head = ds::Expr::Var(next);
                            let body = self.parse_app_rest(body_head);
                            if params.len() == 1 {
                                return ds::Expr::Lam(params.pop().unwrap(), Box::new(body));
                            } else {
                                return ds::Expr::LamN(params, Box::new(body));
                            }
                        }
                    }
                    let body = self.parse_expr();
                    if params.len() == 1 {
                        ds::Expr::Lam(params.pop().unwrap(), Box::new(body))
                    } else {
                        ds::Expr::LamN(params, Box::new(body))
                    }
                } else {
                    self.parse_app_rest(ds::Expr::Var(name))
                }
            }
            _ => self.parse_app(),
        }
    }

    fn parse_let(&mut self) -> ds::Expr {
        self.lexer.expect(&Token::Let);
        if *self.lexer.peek() == Token::Rec {
            self.lexer.next();
            let fname = self.expect_lower_ident();
            let param = self.expect_lower_ident();
            self.lexer.expect(&Token::Eq);
            let body = self.parse_expr();
            self.lexer.expect(&Token::In);
            let rest = self.parse_expr();
            ds::Expr::Letrec(fname, param, Box::new(body), Box::new(rest))
        } else {
            let name = self.expect_lower_ident();
            self.lexer.expect(&Token::Eq);
            let bound = self.parse_expr();
            self.lexer.expect(&Token::In);
            let body = self.parse_expr();
            ds::Expr::Let(name, Box::new(bound), Box::new(body))
        }
    }

    fn parse_match(&mut self) -> ds::Expr {
        self.lexer.expect(&Token::Match);
        let scrutinee = self.parse_app();

        let mut cases = Vec::new();
        while *self.lexer.peek() == Token::Case {
            self.lexer.next();
            let ctor_name = match self.lexer.next() {
                Token::UpperIdent(s) => s,
                tok => panic!("expected constructor name in case, got {tok:?}"),
            };

            let info = self.ctors.get(&ctor_name)
                .unwrap_or_else(|| panic!("unknown constructor: {ctor_name}"));
            let tag = info.tag;
            let arity = info.arity;

            let mut binds = Vec::new();
            if *self.lexer.peek() == Token::LParen {
                self.lexer.next();
                if *self.lexer.peek() != Token::RParen {
                    binds.push(self.expect_lower_ident());
                    while *self.lexer.peek() == Token::Comma {
                        self.lexer.next();
                        binds.push(self.expect_lower_ident());
                    }
                }
                self.lexer.expect(&Token::RParen);
            }

            assert_eq!(
                binds.len(), arity as usize,
                "constructor {ctor_name} has arity {arity}, but {n} binds given",
                n = binds.len()
            );

            self.lexer.expect(&Token::Arrow);
            let body = self.parse_expr();

            cases.push((tag, ds::Case { binds, body }));
        }

        self.lexer.expect(&Token::End);

        let base_tag = cases.iter().map(|(t, _)| *t).min().unwrap_or(0);
        let mut sorted_cases: Vec<ds::Case> = Vec::new();
        for tag in base_tag..base_tag + cases.len() as u8 {
            let (_, case) = cases.iter()
                .find(|(t, _)| *t == tag)
                .unwrap_or_else(|| panic!("missing case for tag {tag}"));
            sorted_cases.push(case.clone());
        }

        ds::Expr::Match(Box::new(scrutinee), base_tag, sorted_cases)
    }

    fn parse_field(&mut self) -> ds::Expr {
        self.lexer.expect(&Token::Field);
        let idx = match self.lexer.next() {
            Token::Number(n) => n as u8,
            tok => panic!("expected field index, got {tok:?}"),
        };
        self.lexer.expect(&Token::Of);
        let expr = self.parse_expr();
        ds::Expr::Field(Box::new(expr), idx)
    }

    fn parse_builtin(&mut self) -> ds::Expr {
        self.lexer.expect(&Token::Builtin);
        let op_name = self.expect_lower_ident();
        let (op, arity) = match op_name.as_str() {
            "add"          => (PrimOp::Int(IntOp::Add), 2),
            "sub"          => (PrimOp::Int(IntOp::Sub), 2),
            "mul"          => (PrimOp::Int(IntOp::Mul), 2),
            "eq"           => (PrimOp::Int(IntOp::Eq), 2),
            "lt"           => (PrimOp::Int(IntOp::Lt), 2),
            "int_byte"     => (PrimOp::Int(IntOp::Byte), 1),
            "bytes_len"    => (PrimOp::Bytes(BytesOp::Len), 1),
            "bytes_get"    => (PrimOp::Bytes(BytesOp::Get), 2),
            "bytes_concat" => (PrimOp::Bytes(BytesOp::Concat), 2),
            "bytes_slice"  => (PrimOp::Bytes(BytesOp::Slice), 3),
            "bytes_eq"     => (PrimOp::Bytes(BytesOp::Eq), 2),
            _ => panic!("unknown builtin operation: {op_name}"),
        };
        let args: Vec<ds::Expr> = (0..arity).map(|_| self.parse_atom()).collect();
        ds::Expr::Prim(op, args)
    }

    fn parse_app(&mut self) -> ds::Expr {
        let head = self.parse_atom();
        self.parse_app_rest(head)
    }

    fn parse_app_rest(&mut self, head: ds::Expr) -> ds::Expr {
        let mut args = Vec::new();
        while self.is_atom_start() {
            args.push(self.parse_atom());
        }
        match args.len() {
            0 => head,
            1 => ds::Expr::App(Box::new(head), Box::new(args.pop().unwrap())),
            _ => ds::Expr::AppN(Box::new(head), args),
        }
    }

    fn is_atom_start(&mut self) -> bool {
        matches!(
            self.lexer.peek(),
            Token::LowerIdent(_) | Token::UpperIdent(_) | Token::Number(_)
                | Token::StringLit(_) | Token::LParen
        )
    }

    fn parse_atom(&mut self) -> ds::Expr {
        match self.lexer.peek() {
            Token::LowerIdent(_) => {
                let name = self.expect_lower_ident();
                ds::Expr::Var(name)
            }
            Token::UpperIdent(_) => self.parse_ctor(),
            Token::Number(_) => {
                let n = match self.lexer.next() {
                    Token::Number(n) => n,
                    _ => unreachable!(),
                };
                ds::Expr::Int(n as i32)
            }
            Token::StringLit(_) => {
                let data = match self.lexer.next() {
                    Token::StringLit(data) => data,
                    _ => unreachable!(),
                };
                ds::Expr::Bytes(data)
            }
            Token::LParen => {
                self.lexer.next();
                let expr = self.parse_expr();
                self.lexer.expect(&Token::RParen);
                expr
            }
            tok => panic!("expected expression, got {tok:?}"),
        }
    }

    fn parse_ctor(&mut self) -> ds::Expr {
        let name = match self.lexer.next() {
            Token::UpperIdent(s) => s,
            tok => panic!("expected constructor name, got {tok:?}"),
        };

        let info = self.ctors.get(&name)
            .unwrap_or_else(|| panic!("unknown constructor: {name}"));
        let tag = info.tag;
        let arity = info.arity;

        let mut fields = Vec::new();
        if arity > 0 {
            self.lexer.expect(&Token::LParen);
            fields.push(self.parse_expr());
            while *self.lexer.peek() == Token::Comma {
                self.lexer.next();
                fields.push(self.parse_expr());
            }
            self.lexer.expect(&Token::RParen);
        }

        assert_eq!(
            fields.len(), arity as usize,
            "constructor {name} expects {arity} fields, got {n}",
            n = fields.len()
        );

        ds::Expr::Ctor(tag, fields)
    }
}


