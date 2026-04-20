use crate::ds;
use crate::prim::{PrimOp, IntOp, BytesOp};
use crate::lexer::{TokenStream, Token};
use encore_compiler::frontend::{CtorRegistry, ParseError};

pub struct Parser {
    tokens: TokenStream,
    ctors: CtorRegistry,
}

impl Parser {
    pub fn new(input: &str) -> Self {
        Self {
            tokens: TokenStream::new(input),
            ctors: CtorRegistry::new(),
        }
    }

    pub fn parse_module(&mut self) -> Result<ds::Module, ParseError> {
        let mut defines = Vec::new();
        while *self.tokens.peek()? != Token::Eof {
            if *self.tokens.peek()? == Token::Data {
                self.parse_data()?;
            } else {
                defines.push(self.parse_define()?);
            }
        }
        Ok(ds::Module { defines })
    }

    pub fn ctor_names(&self) -> Vec<(u8, String)> {
        self.ctors.ctor_names()
    }

    // -- Data declarations --

    fn parse_data(&mut self) -> Result<(), ParseError> {
        self.tokens.expect(&Token::Data)?;

        let type_id = self.ctors.alloc_type_id();

        self.tokens.try_consume(&Token::Pipe)?;

        self.parse_variant(type_id)?;

        while self.tokens.try_consume(&Token::Pipe)? {
            self.parse_variant(type_id)?;
        }
        Ok(())
    }

    fn parse_variant(&mut self, type_id: u8) -> Result<(), ParseError> {
        let name = self.tokens.expect_upper_identifier()?;
        let binds = self.parse_paren_binds()?;
        self.ctors.resolve_with_type(&name, binds.len() as u8, type_id);
        Ok(())
    }

    // -- Shared helpers --

    fn parse_paren_binds(&mut self) -> Result<Vec<String>, ParseError> {
        if !self.tokens.try_consume(&Token::LParen)? {
            return Ok(Vec::new());
        }
        let binds = if *self.tokens.peek()? == Token::RParen {
            Vec::new()
        } else {
            self.tokens.comma_separated(|ts| ts.expect_lower_identifier())?
        };
        self.tokens.expect(&Token::RParen)?;
        Ok(binds)
    }

    fn build_match(
        scrutinee: ds::Expr,
        cases: &[(u8, ds::Case)],
        type_ctors: &[(u8, u8)],
        wildcard: Option<&ds::Expr>,
    ) -> ds::Expr {
        let base_tag = type_ctors.iter().map(|&(t, _)| t).min().unwrap_or(0);
        let sorted_cases: Vec<ds::Case> = type_ctors.iter().map(|&(tag, arity)| {
            if let Some((_, case)) = cases.iter().find(|(t, _)| *t == tag) {
                case.clone()
            } else {
                ds::Case {
                    binds: vec!["_".to_string(); arity as usize],
                    body: wildcard.unwrap().clone(),
                }
            }
        }).collect();
        ds::Expr::Match(Box::new(scrutinee), base_tag, sorted_cases)
    }

    // -- Defines --

    fn parse_define(&mut self) -> Result<ds::Define, ParseError> {
        self.tokens.expect(&Token::Let)?;
        if self.tokens.try_consume(&Token::Extern)? {
            let name = self.tokens.expect_lower_identifier()?;
            let slot = self.tokens.expect_number()? as u16;
            return Ok(ds::Define { name, body: ds::Expr::Extern(slot) });
        }
        if self.tokens.try_consume(&Token::Rec)? {
            let name = self.tokens.expect_lower_identifier()?;
            let param = self.tokens.expect_lower_identifier()?;
            self.tokens.expect(&Token::Eq)?;
            let body = self.parse_expr()?;
            return Ok(ds::Define {
                name,
                body: ds::Expr::Lambda(vec![param], Box::new(body)),
            });
        }
        let name = self.tokens.expect_lower_identifier()?;
        self.tokens.expect(&Token::Eq)?;
        let body = self.parse_expr()?;
        Ok(ds::Define { name, body })
    }

    // -- Expressions --

    fn parse_expr(&mut self) -> Result<ds::Expr, ParseError> {
        match self.tokens.peek()? {
            Token::Let => self.parse_let(),
            Token::Match => self.parse_match(),
            Token::Field => self.parse_field(),
            Token::Builtin => self.parse_builtin(),
            Token::If => self.parse_if_binding(),
            Token::LowerIdent(_) => {
                let name = self.tokens.expect_lower_identifier()?;
                if *self.tokens.peek()? == Token::Arrow {
                    self.tokens.next()?;
                    let mut params = vec![name];
                    while let Token::LowerIdent(_) = self.tokens.peek()? {
                        let next = self.tokens.expect_lower_identifier()?;
                        if *self.tokens.peek()? == Token::Arrow {
                            self.tokens.next()?;
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
        self.tokens.expect(&Token::Let)?;
        if self.tokens.try_consume(&Token::Rec)? {
            let fname = self.tokens.expect_lower_identifier()?;
            let param = self.tokens.expect_lower_identifier()?;
            self.tokens.expect(&Token::Eq)?;
            let body = self.parse_expr()?;
            self.tokens.expect(&Token::In)?;
            let rest = self.parse_expr()?;
            Ok(ds::Expr::Letrec(fname, param, Box::new(body), Box::new(rest)))
        } else if matches!(self.tokens.peek()?, Token::UpperIdent(_)) {
            self.parse_let_destruct_chain()
        } else {
            let name = self.tokens.expect_lower_identifier()?;
            self.tokens.expect(&Token::Eq)?;
            let bound = self.parse_expr()?;
            let mut bindings = vec![(name, bound)];
            while self.tokens.try_consume(&Token::Comma)? {
                let n = self.tokens.expect_lower_identifier()?;
                self.tokens.expect(&Token::Eq)?;
                let b = self.parse_expr()?;
                bindings.push((n, b));
            }
            self.tokens.expect(&Token::In)?;
            let body = self.parse_expr()?;
            Ok(bindings.into_iter().rev().fold(body, |acc, (n, b)| {
                ds::Expr::Let(n, Box::new(b), Box::new(acc))
            }))
        }
    }

    fn parse_ctor_binding(&mut self) -> Result<(u8, u8, Vec<String>, ds::Expr), ParseError> {
        let ctor_name = self.tokens.expect_upper_identifier()?;
        let (tag, arity, type_id) = self.ctors.get_with_type(&ctor_name)
            .ok_or_else(|| ParseError::from(format!("unknown constructor: {ctor_name}")))?;

        let binds = self.parse_paren_binds()?;
        if binds.len() != arity as usize {
            return Err(format!(
                "constructor {ctor_name} has arity {arity}, but {} binds given",
                binds.len()
            ).into());
        }

        self.tokens.expect(&Token::Eq)?;
        let scrutinee = self.parse_expr()?;

        Ok((tag, type_id, binds, scrutinee))
    }

    fn parse_let_destruct_chain(&mut self) -> Result<ds::Expr, ParseError> {
        let mut bindings: Vec<(u8, Vec<String>, ds::Expr)> = Vec::new();

        loop {
            let (tag, _type_id, binds, scrutinee) = self.parse_ctor_binding()?;
            bindings.push((tag, binds, scrutinee));
            if !self.tokens.try_consume(&Token::Comma)? { break; }
        }

        self.tokens.expect(&Token::In)?;
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
        self.tokens.expect(&Token::If)?;

        let mut raw_bindings: Vec<(u8, u8, Vec<String>, ds::Expr)> = Vec::new();
        loop {
            raw_bindings.push(self.parse_ctor_binding()?);
            if !self.tokens.try_consume(&Token::Comma)? { break; }
        }

        self.tokens.expect(&Token::Then)?;
        let body = self.parse_expr()?;
        self.tokens.expect(&Token::Else)?;
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
        self.tokens.expect(&Token::Match)?;
        let scrutinee = self.parse_app()?;

        let mut cases: Vec<(u8, ds::Case)> = Vec::new();
        let mut wildcard: Option<ds::Expr> = None;
        let mut type_id: Option<u8> = None;

        while self.tokens.try_consume(&Token::Pipe)? {
            if matches!(self.tokens.peek()?, Token::LowerIdent(s) if s == "_") {
                self.tokens.next()?;
                self.tokens.expect(&Token::Arrow)?;
                let body = self.parse_expr()?;
                if wildcard.is_some() {
                    return Err("duplicate wildcard case in match".into());
                }
                wildcard = Some(body);
                continue;
            }

            if wildcard.is_some() {
                return Err("no case branches allowed after wildcard `_`".into());
            }

            let ctor_name = self.tokens.expect_upper_identifier()?;
            let (tag, arity, tid) = self.ctors.get_with_type(&ctor_name)
                .ok_or_else(|| ParseError::from(format!("unknown constructor: {ctor_name}")))?;
            if let Some(prev) = type_id {
                if prev != tid {
                    return Err("match branches mix constructors from different types".into());
                }
            }
            type_id = Some(tid);

            let binds = self.parse_paren_binds()?;
            if binds.len() != arity as usize {
                return Err(format!(
                    "constructor {ctor_name} has arity {arity}, but {} binds given",
                    binds.len()
                ).into());
            }

            self.tokens.expect(&Token::Arrow)?;
            let body = self.parse_expr()?;

            cases.push((tag, ds::Case { binds, body }));
        }

        self.tokens.expect(&Token::End)?;

        let tid = type_id
            .ok_or_else(|| ParseError::from("match requires at least one constructor branch"))?;
        let type_ctors = self.ctors.ctors_of_type(tid);

        if wildcard.is_none() {
            let missing: Vec<&str> = type_ctors.iter()
                .filter(|(tag, _)| !cases.iter().any(|(t, _)| *t == *tag))
                .filter_map(|(tag, _)| self.ctors.name_of_tag(*tag))
                .collect();
            if !missing.is_empty() {
                return Err(format!(
                    "non-exhaustive match: missing constructor(s) {}",
                    missing.join(", ")
                ).into());
            }
        }

        Ok(Self::build_match(scrutinee, &cases, &type_ctors, wildcard.as_ref()))
    }

    fn parse_field(&mut self) -> Result<ds::Expr, ParseError> {
        self.tokens.expect(&Token::Field)?;
        let idx = self.tokens.expect_number()? as u8;
        self.tokens.expect(&Token::Of)?;
        let expr = self.parse_expr()?;
        Ok(ds::Expr::Field(Box::new(expr), idx))
    }

    fn parse_builtin(&mut self) -> Result<ds::Expr, ParseError> {
        self.tokens.expect(&Token::Builtin)?;
        let op_name = self.tokens.expect_lower_identifier()?;
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

    // -- Application / atoms --

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
            self.tokens.peek()?,
            Token::LowerIdent(_) | Token::UpperIdent(_) | Token::Number(_)
                | Token::StringLit(_) | Token::LParen
        ))
    }

    fn parse_atom(&mut self) -> Result<ds::Expr, ParseError> {
        match self.tokens.peek()? {
            Token::LowerIdent(_) => {
                let name = self.tokens.expect_lower_identifier()?;
                Ok(ds::Expr::Var(name))
            }
            Token::UpperIdent(_) => self.parse_ctor(),
            Token::Number(_) => {
                Ok(ds::Expr::Int(self.tokens.expect_number()? as i32))
            }
            Token::StringLit(_) => {
                match self.tokens.next()? {
                    Token::StringLit(data) => Ok(ds::Expr::Bytes(data)),
                    _ => unreachable!(),
                }
            }
            Token::LParen => {
                self.tokens.next()?;
                let expr = self.parse_expr()?;
                self.tokens.expect(&Token::RParen)?;
                Ok(expr)
            }
            tok => Err(format!("expected expression, got {tok:?}").into()),
        }
    }

    fn parse_ctor(&mut self) -> Result<ds::Expr, ParseError> {
        let name = self.tokens.expect_upper_identifier()?;

        let (tag, arity) = self.ctors.get(&name)
            .ok_or_else(|| ParseError::from(format!("unknown constructor: {name}")))?;

        let mut fields = Vec::new();
        if arity > 0 {
            self.tokens.expect(&Token::LParen)?;
            fields.push(self.parse_expr()?);
            while self.tokens.try_consume(&Token::Comma)? {
                fields.push(self.parse_expr()?);
            }
            self.tokens.expect(&Token::RParen)?;
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
