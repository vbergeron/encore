use crate::ds;
use crate::prim::{PrimOp, IntOp, BytesOp};
use crate::lexer::{Lexer, Token};
use encore_compiler::frontend::{CtorRegistry, ParseError};

pub struct Parser {
    lexer: Lexer,
    ctors: CtorRegistry,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        Self {
            lexer: Lexer::new(input),
            ctors: CtorRegistry::new(),
        }
    }

    pub fn parse_module(&mut self) -> Result<ds::Module, ParseError> {
        while *self.lexer.peek()? == Token::Data {
            self.parse_data()?;
        }

        let mut defines = Vec::new();
        while *self.lexer.peek()? != Token::Eof {
            defines.push(self.parse_define()?);
        }

        Ok(ds::Module { defines })
    }

    pub fn ctor_names(&self) -> Vec<(u8, String)> {
        self.ctors.ctor_names()
    }

    fn parse_data(&mut self) -> Result<(), ParseError> {
        self.lexer.expect(&Token::Data)?;

        let type_id = self.ctors.alloc_type_id();

        if *self.lexer.peek()? == Token::Pipe {
            self.lexer.next()?;
        }

        self.parse_variant(type_id)?;

        while *self.lexer.peek()? == Token::Pipe {
            self.lexer.next()?;
            self.parse_variant(type_id)?;
        }
        Ok(())
    }

    fn parse_variant(&mut self, type_id: u8) -> Result<(), ParseError> {
        let name = match self.lexer.next()? {
            Token::UpperIdent(s) => s,
            tok => return Err(format!("expected constructor name, got {tok:?}").into()),
        };

        let mut arity: u8 = 0;
        if *self.lexer.peek()? == Token::LParen {
            self.lexer.next()?;
            if *self.lexer.peek()? != Token::RParen {
                self.expect_lower_ident()?;
                arity = 1;
                while *self.lexer.peek()? == Token::Comma {
                    self.lexer.next()?;
                    self.expect_lower_ident()?;
                    arity += 1;
                }
            }
            self.lexer.expect(&Token::RParen)?;
        }

        self.ctors.resolve_with_type(&name, arity, type_id);
        Ok(())
    }

    fn expect_lower_ident(&mut self) -> Result<String, ParseError> {
        match self.lexer.next()? {
            Token::LowerIdent(s) => Ok(s),
            tok => Err(format!("expected identifier, got {tok:?}").into()),
        }
    }

    fn parse_define(&mut self) -> Result<ds::Define, ParseError> {
        self.lexer.expect(&Token::Define)?;
        if *self.lexer.peek()? == Token::Extern {
            self.lexer.next()?;
            let name = self.expect_lower_ident()?;
            let slot = match self.lexer.next()? {
                Token::Number(n) => n as u16,
                tok => return Err(format!("expected extern slot index, got {tok:?}").into()),
            };
            return Ok(ds::Define { name, body: ds::Expr::Extern(slot) });
        }
        let name = self.expect_lower_ident()?;
        self.lexer.expect(&Token::As)?;
        let body = self.parse_expr()?;
        Ok(ds::Define { name, body })
    }

    fn parse_expr(&mut self) -> Result<ds::Expr, ParseError> {
        match self.lexer.peek()? {
            Token::Let => self.parse_let(),
            Token::Match => self.parse_match(),
            Token::Field => self.parse_field(),
            Token::Builtin => self.parse_builtin(),
            Token::If => self.parse_if_binding(),
            Token::LowerIdent(_) => {
                let name = self.expect_lower_ident()?;
                if *self.lexer.peek()? == Token::Arrow {
                    self.lexer.next()?;
                    let mut params = vec![name];
                    while let Token::LowerIdent(_) = self.lexer.peek()? {
                        let next = self.expect_lower_ident()?;
                        if *self.lexer.peek()? == Token::Arrow {
                            self.lexer.next()?;
                            params.push(next);
                        } else {
                            let body_head = ds::Expr::Var(next);
                            let body = self.parse_app_rest(body_head)?;
                            return Ok(params.into_iter().rev().fold(body, |b, p| {
                                ds::Expr::Lambda(vec![p], Box::new(b))
                            }));
                        }
                    }
                    let body = self.parse_expr()?;
                    Ok(params.into_iter().rev().fold(body, |b, p| {
                        ds::Expr::Lambda(vec![p], Box::new(b))
                    }))
                } else {
                    self.parse_app_rest(ds::Expr::Var(name))
                }
            }
            _ => self.parse_app(),
        }
    }

    fn parse_let(&mut self) -> Result<ds::Expr, ParseError> {
        self.lexer.expect(&Token::Let)?;
        if *self.lexer.peek()? == Token::Rec {
            self.lexer.next()?;
            let fname = self.expect_lower_ident()?;
            let param = self.expect_lower_ident()?;
            self.lexer.expect(&Token::Eq)?;
            let body = self.parse_expr()?;
            self.lexer.expect(&Token::In)?;
            let rest = self.parse_expr()?;
            Ok(ds::Expr::Letrec(fname, param, Box::new(body), Box::new(rest)))
        } else if matches!(self.lexer.peek()?, Token::UpperIdent(_)) {
            self.parse_let_destruct_chain()
        } else {
            let name = self.expect_lower_ident()?;
            self.lexer.expect(&Token::Eq)?;
            let bound = self.parse_expr()?;
            let mut bindings = vec![(name, bound)];
            while *self.lexer.peek()? == Token::Comma {
                self.lexer.next()?;
                let n = self.expect_lower_ident()?;
                self.lexer.expect(&Token::Eq)?;
                let b = self.parse_expr()?;
                bindings.push((n, b));
            }
            self.lexer.expect(&Token::In)?;
            let body = self.parse_expr()?;
            Ok(bindings.into_iter().rev().fold(body, |acc, (n, b)| {
                ds::Expr::Let(n, Box::new(b), Box::new(acc))
            }))
        }
    }

    fn parse_ctor_binding(&mut self) -> Result<(u8, u8, Vec<String>, ds::Expr), ParseError> {
        let ctor_name = match self.lexer.next()? {
            Token::UpperIdent(s) => s,
            tok => return Err(format!("expected constructor name in pattern binding, got {tok:?}").into()),
        };
        let (tag, arity, type_id) = self.ctors.get_with_type(&ctor_name)
            .ok_or_else(|| ParseError::from(format!("unknown constructor: {ctor_name}")))?;

        let mut binds = Vec::new();
        if *self.lexer.peek()? == Token::LParen {
            self.lexer.next()?;
            if *self.lexer.peek()? != Token::RParen {
                binds.push(self.expect_lower_ident()?);
                while *self.lexer.peek()? == Token::Comma {
                    self.lexer.next()?;
                    binds.push(self.expect_lower_ident()?);
                }
            }
            self.lexer.expect(&Token::RParen)?;
        }

        if binds.len() != arity as usize {
            return Err(format!(
                "constructor {ctor_name} has arity {arity}, but {} binds given",
                binds.len()
            ).into());
        }

        self.lexer.expect(&Token::Eq)?;
        let scrutinee = self.parse_expr()?;

        Ok((tag, type_id, binds, scrutinee))
    }

    fn parse_let_destruct_chain(&mut self) -> Result<ds::Expr, ParseError> {
        let mut bindings: Vec<(u8, Vec<String>, ds::Expr)> = Vec::new();

        loop {
            let (tag, _type_id, binds, scrutinee) = self.parse_ctor_binding()?;
            bindings.push((tag, binds, scrutinee));
            if *self.lexer.peek()? == Token::Comma {
                self.lexer.next()?;
            } else {
                break;
            }
        }

        self.lexer.expect(&Token::In)?;
        let body = self.parse_expr()?;

        Ok(bindings.into_iter().rev().fold(body, |acc, (tag, binds, scrutinee)| {
            ds::Expr::Match(
                Box::new(scrutinee),
                tag,
                vec![ds::Case { binds, body: acc }],
            )
        }))
    }

    fn parse_if_binding(&mut self) -> Result<ds::Expr, ParseError> {
        self.lexer.expect(&Token::If)?;

        let mut raw_bindings: Vec<(u8, u8, Vec<String>, ds::Expr)> = Vec::new();
        loop {
            raw_bindings.push(self.parse_ctor_binding()?);
            if *self.lexer.peek()? == Token::Comma {
                self.lexer.next()?;
            } else {
                break;
            }
        }

        self.lexer.expect(&Token::Then)?;
        let body = self.parse_expr()?;
        self.lexer.expect(&Token::Else)?;
        let alt = self.parse_expr()?;

        let bindings: Vec<(u8, Vec<(u8, u8)>, Vec<String>, ds::Expr)> = raw_bindings
            .into_iter()
            .map(|(tag, type_id, binds, scrutinee)| {
                (tag, self.ctors.ctors_of_type(type_id), binds, scrutinee)
            })
            .collect();

        Ok(bindings.into_iter().rev().fold(body, |acc, (tag, type_ctors, binds, scrutinee)| {
            let base_tag = type_ctors.iter().map(|&(t, _)| t).min().unwrap_or(tag);
            let matched_idx = type_ctors.iter().position(|&(t, _)| t == tag).unwrap();
            let mut cases: Vec<ds::Case> = type_ctors.iter().map(|&(_, arity)| {
                ds::Case {
                    binds: vec!["_".to_string(); arity as usize],
                    body: alt.clone(),
                }
            }).collect();
            cases[matched_idx] = ds::Case { binds, body: acc };
            ds::Expr::Match(Box::new(scrutinee), base_tag, cases)
        }))
    }

    fn parse_match(&mut self) -> Result<ds::Expr, ParseError> {
        self.lexer.expect(&Token::Match)?;
        let scrutinee = self.parse_app()?;

        let mut cases = Vec::new();
        while *self.lexer.peek()? == Token::Case {
            self.lexer.next()?;
            let ctor_name = match self.lexer.next()? {
                Token::UpperIdent(s) => s,
                tok => return Err(format!("expected constructor name in case, got {tok:?}").into()),
            };

            let (tag, arity) = self.ctors.get(&ctor_name)
                .ok_or_else(|| ParseError::from(format!("unknown constructor: {ctor_name}")))?;

            let mut binds = Vec::new();
            if *self.lexer.peek()? == Token::LParen {
                self.lexer.next()?;
                if *self.lexer.peek()? != Token::RParen {
                    binds.push(self.expect_lower_ident()?);
                    while *self.lexer.peek()? == Token::Comma {
                        self.lexer.next()?;
                        binds.push(self.expect_lower_ident()?);
                    }
                }
                self.lexer.expect(&Token::RParen)?;
            }

            if binds.len() != arity as usize {
                return Err(format!(
                    "constructor {ctor_name} has arity {arity}, but {} binds given",
                    binds.len()
                ).into());
            }

            self.lexer.expect(&Token::Arrow)?;
            let body = self.parse_expr()?;

            cases.push((tag, ds::Case { binds, body }));
        }

        self.lexer.expect(&Token::End)?;

        let base_tag = cases.iter().map(|(t, _)| *t).min().unwrap_or(0);
        let mut sorted_cases: Vec<ds::Case> = Vec::new();
        for tag in base_tag..base_tag + cases.len() as u8 {
            let (_, case) = cases.iter()
                .find(|(t, _)| *t == tag)
                .ok_or_else(|| ParseError::from(format!("missing case for tag {tag}")))?;
            sorted_cases.push(case.clone());
        }

        Ok(ds::Expr::Match(Box::new(scrutinee), base_tag, sorted_cases))
    }

    fn parse_field(&mut self) -> Result<ds::Expr, ParseError> {
        self.lexer.expect(&Token::Field)?;
        let idx = match self.lexer.next()? {
            Token::Number(n) => n as u8,
            tok => return Err(format!("expected field index, got {tok:?}").into()),
        };
        self.lexer.expect(&Token::Of)?;
        let expr = self.parse_expr()?;
        Ok(ds::Expr::Field(Box::new(expr), idx))
    }

    fn parse_builtin(&mut self) -> Result<ds::Expr, ParseError> {
        self.lexer.expect(&Token::Builtin)?;
        let op_name = self.expect_lower_ident()?;
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
            _ => return Err(format!("unknown builtin operation: {op_name}").into()),
        };
        let args: Vec<ds::Expr> = (0..arity)
            .map(|_| self.parse_atom())
            .collect::<Result<_, _>>()?;
        Ok(ds::Expr::Prim(op, args))
    }

    fn parse_app(&mut self) -> Result<ds::Expr, ParseError> {
        let head = self.parse_atom()?;
        self.parse_app_rest(head)
    }

    fn parse_app_rest(&mut self, head: ds::Expr) -> Result<ds::Expr, ParseError> {
        let mut args = Vec::new();
        while self.is_atom_start()? {
            args.push(self.parse_atom()?);
        }
        if args.is_empty() {
            Ok(head)
        } else {
            Ok(ds::Expr::Apply(Box::new(head), args))
        }
    }

    fn is_atom_start(&mut self) -> Result<bool, ParseError> {
        Ok(matches!(
            self.lexer.peek()?,
            Token::LowerIdent(_) | Token::UpperIdent(_) | Token::Number(_)
                | Token::StringLit(_) | Token::LParen
        ))
    }

    fn parse_atom(&mut self) -> Result<ds::Expr, ParseError> {
        match self.lexer.peek()? {
            Token::LowerIdent(_) => {
                let name = self.expect_lower_ident()?;
                Ok(ds::Expr::Var(name))
            }
            Token::UpperIdent(_) => self.parse_ctor(),
            Token::Number(_) => {
                let n = match self.lexer.next()? {
                    Token::Number(n) => n,
                    _ => unreachable!(),
                };
                Ok(ds::Expr::Int(n as i32))
            }
            Token::StringLit(_) => {
                let data = match self.lexer.next()? {
                    Token::StringLit(data) => data,
                    _ => unreachable!(),
                };
                Ok(ds::Expr::Bytes(data))
            }
            Token::LParen => {
                self.lexer.next()?;
                let expr = self.parse_expr()?;
                self.lexer.expect(&Token::RParen)?;
                Ok(expr)
            }
            tok => Err(format!("expected expression, got {tok:?}").into()),
        }
    }

    fn parse_ctor(&mut self) -> Result<ds::Expr, ParseError> {
        let name = match self.lexer.next()? {
            Token::UpperIdent(s) => s,
            tok => return Err(format!("expected constructor name, got {tok:?}").into()),
        };

        let (tag, arity) = self.ctors.get(&name)
            .ok_or_else(|| ParseError::from(format!("unknown constructor: {name}")))?;

        let mut fields = Vec::new();
        if arity > 0 {
            self.lexer.expect(&Token::LParen)?;
            fields.push(self.parse_expr()?);
            while *self.lexer.peek()? == Token::Comma {
                self.lexer.next()?;
                fields.push(self.parse_expr()?);
            }
            self.lexer.expect(&Token::RParen)?;
        }

        if fields.len() != arity as usize {
            return Err(format!(
                "constructor {name} expects {arity} fields, got {}",
                fields.len()
            ).into());
        }

        Ok(ds::Expr::Ctor(tag, fields))
    }
}
