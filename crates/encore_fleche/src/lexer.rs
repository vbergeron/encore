use encore_compiler::frontend::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LowerIdent(String),
    UpperIdent(String),
    Number(u64),
    Arrow,
    Eq,
    Pipe,
    LParen,
    RParen,
    Comma,
    Data,
    Define,
    As,
    Let,
    In,
    Rec,
    Match,
    Case,
    End,
    Field,
    Of,
    Builtin,
    Extern,
    If,
    Then,
    Else,
    StringLit(Vec<u8>),
    Eof,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    peeked: Option<Token>,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            peeked: None,
        }
    }

    pub fn peek(&mut self) -> Result<&Token, ParseError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.read_token()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    pub fn next(&mut self) -> Result<Token, ParseError> {
        if let Some(tok) = self.peeked.take() {
            return Ok(tok);
        }
        self.read_token()
    }

    pub fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        let tok = self.next()?;
        if &tok != expected {
            return Err(ParseError { message: format!("expected {expected:?}, got {tok:?}") });
        }
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        loop {
            while self.pos < self.input.len() && self.input[self.pos].is_whitespace() {
                self.pos += 1;
            }
            if self.pos < self.input.len() && self.input[self.pos] == '#' {
                while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn read_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();

        if self.pos >= self.input.len() {
            return Ok(Token::Eof);
        }

        let ch = self.input[self.pos];

        if ch == '-' && self.pos + 1 < self.input.len() && self.input[self.pos + 1] == '>' {
            self.pos += 2;
            return Ok(Token::Arrow);
        }

        match ch {
            '=' => { self.pos += 1; Ok(Token::Eq) }
            '|' => { self.pos += 1; Ok(Token::Pipe) }
            '(' => { self.pos += 1; Ok(Token::LParen) }
            ')' => { self.pos += 1; Ok(Token::RParen) }
            ',' => { self.pos += 1; Ok(Token::Comma) }
            '"' => self.read_string(),
            '0'..='9' => Ok(self.read_number()),
            c if c.is_alphabetic() || c == '_' => Ok(self.read_ident()),
            c => Err(ParseError { message: format!("unexpected character: {c:?}") }),
        }
    }

    fn read_string(&mut self) -> Result<Token, ParseError> {
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos] != '"' {
            self.pos += 1;
        }
        if self.pos >= self.input.len() {
            return Err(ParseError { message: "unterminated string literal".into() });
        }
        let bytes: Vec<u8> = self.input[start..self.pos].iter().map(|&c| c as u8).collect();
        self.pos += 1;
        Ok(Token::StringLit(bytes))
    }

    fn read_number(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let s: String = self.input[start..self.pos].iter().collect();
        Token::Number(s.parse().unwrap())
    }

    fn read_ident(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.input.len()
            && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_')
        {
            self.pos += 1;
        }
        let s: String = self.input[start..self.pos].iter().collect();

        match s.as_str() {
            "data" => Token::Data,
            "define" => Token::Define,
            "as" => Token::As,
            "let" => Token::Let,
            "in" => Token::In,
            "rec" => Token::Rec,
            "match" => Token::Match,
            "case" => Token::Case,
            "end" => Token::End,
            "field" => Token::Field,
            "of" => Token::Of,
            "builtin" => Token::Builtin,
            "extern" => Token::Extern,
            "if" => Token::If,
            "then" => Token::Then,
            "else" => Token::Else,
            _ if s.chars().next().unwrap().is_uppercase() => Token::UpperIdent(s),
            _ => Token::LowerIdent(s),
        }
    }
}
