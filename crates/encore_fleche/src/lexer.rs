use encore_compiler::frontend::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LowerIdent(String),
    UpperIdent(String),
    Number(i64),
    Arrow,
    Eq,
    Pipe,
    LParen,
    RParen,
    Comma,
    Data,
    Let,
    In,
    Rec,
    Match,
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

struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    fn new(input: &str) -> Self {
        Self { input: input.chars().collect(), pos: 0 }
    }

    fn read_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();

        if self.pos >= self.input.len() {
            return Ok(Token::Eof);
        }

        match self.input[self.pos] {
            '-' if self.lookahead() == Some('>') => { self.pos += 2; Ok(Token::Arrow) }
            '-' if self.lookahead().map_or(false, |c| c.is_ascii_digit()) => {
                self.pos += 1;
                Ok(self.read_number(-1))
            }
            '=' => { self.pos += 1; Ok(Token::Eq) }
            '|' => { self.pos += 1; Ok(Token::Pipe) }
            '(' => { self.pos += 1; Ok(Token::LParen) }
            ')' => { self.pos += 1; Ok(Token::RParen) }
            ',' => { self.pos += 1; Ok(Token::Comma) }
            '"' => self.read_string(),
            '0'..='9' => Ok(self.read_number(1)),
            c if c.is_alphabetic() || c == '_' => Ok(self.read_ident()),
            c => Err(ParseError { message: format!("unexpected character: {c:?}") }),
        }
    }

    fn lookahead(&self) -> Option<char> {
        self.input.get(self.pos + 1).copied()
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

    fn read_string(&mut self) -> Result<Token, ParseError> {
        self.pos += 1;
        let mut bytes = Vec::new();
        while self.pos < self.input.len() && self.input[self.pos] != '"' {
            if self.input[self.pos] == '\\' {
                self.pos += 1;
                if self.pos >= self.input.len() {
                    return Err(ParseError { message: "unterminated escape in string literal".into() });
                }
                let escaped = match self.input[self.pos] {
                    'n' => b'\n',
                    't' => b'\t',
                    '0' => b'\0',
                    '\\' => b'\\',
                    '"' => b'"',
                    c => return Err(ParseError { message: format!("unknown escape sequence: \\{c}") }),
                };
                bytes.push(escaped);
            } else {
                bytes.push(self.input[self.pos] as u8);
            }
            self.pos += 1;
        }
        if self.pos >= self.input.len() {
            return Err(ParseError { message: "unterminated string literal".into() });
        }
        self.pos += 1;
        Ok(Token::StringLit(bytes))
    }

    fn read_number(&mut self, sign: i64) -> Token {
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        let s: String = self.input[start..self.pos].iter().collect();
        Token::Number(sign * s.parse::<i64>().unwrap())
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
            "let" => Token::Let,
            "in" => Token::In,
            "rec" => Token::Rec,
            "match" => Token::Match,
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

// -- TokenStream: parser-facing API over the raw Lexer --

pub struct TokenStream {
    lexer: Lexer,
    peeked: Option<Token>,
}

impl TokenStream {
    pub fn new(input: &str) -> Self {
        Self { lexer: Lexer::new(input), peeked: None }
    }

    pub fn peek(&mut self) -> Result<&Token, ParseError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lexer.read_token()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    pub fn next(&mut self) -> Result<Token, ParseError> {
        if let Some(tok) = self.peeked.take() {
            return Ok(tok);
        }
        self.lexer.read_token()
    }

    pub fn try_consume(&mut self, expected: &Token) -> Result<bool, ParseError> {
        if self.peek()? == expected {
            self.next()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn expect(&mut self, expected: &Token) -> Result<(), ParseError> {
        let tok = self.next()?;
        if &tok != expected {
            return Err(ParseError { message: format!("expected {expected:?}, got {tok:?}") });
        }
        Ok(())
    }

    pub fn expect_upper_identifier(&mut self) -> Result<String, ParseError> {
        match self.next()? {
            Token::UpperIdent(s) => Ok(s),
            tok => Err(format!("expected upper-case identifier, got {tok:?}").into()),
        }
    }

    pub fn expect_lower_identifier(&mut self) -> Result<String, ParseError> {
        match self.next()? {
            Token::LowerIdent(s) => Ok(s),
            tok => Err(format!("expected identifier, got {tok:?}").into()),
        }
    }

    pub fn expect_number(&mut self) -> Result<i64, ParseError> {
        match self.next()? {
            Token::Number(n) => Ok(n),
            tok => Err(format!("expected number, got {tok:?}").into()),
        }
    }

    pub fn comma_separated<T>(
        &mut self,
        mut parse_item: impl FnMut(&mut Self) -> Result<T, ParseError>,
    ) -> Result<Vec<T>, ParseError> {
        let mut items = vec![parse_item(self)?];
        while self.try_consume(&Token::Comma)? {
            items.push(parse_item(self)?);
        }
        Ok(items)
    }
}
