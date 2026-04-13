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

    pub fn peek(&mut self) -> &Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.read_token());
        }
        self.peeked.as_ref().unwrap()
    }

    pub fn next(&mut self) -> Token {
        if let Some(tok) = self.peeked.take() {
            return tok;
        }
        self.read_token()
    }

    pub fn expect(&mut self, expected: &Token) {
        let tok = self.next();
        assert_eq!(&tok, expected, "expected {expected:?}, got {tok:?}");
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

    fn read_token(&mut self) -> Token {
        self.skip_whitespace();

        if self.pos >= self.input.len() {
            return Token::Eof;
        }

        let ch = self.input[self.pos];

        if ch == '-' && self.pos + 1 < self.input.len() && self.input[self.pos + 1] == '>' {
            self.pos += 2;
            return Token::Arrow;
        }

        match ch {
            '=' => { self.pos += 1; Token::Eq }
            '|' => { self.pos += 1; Token::Pipe }
            '(' => { self.pos += 1; Token::LParen }
            ')' => { self.pos += 1; Token::RParen }
            ',' => { self.pos += 1; Token::Comma }
            '0'..='9' => self.read_number(),
            c if c.is_alphabetic() || c == '_' => self.read_ident(),
            c => panic!("unexpected character: {c:?}"),
        }
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
            _ if s.chars().next().unwrap().is_uppercase() => Token::UpperIdent(s),
            _ => Token::LowerIdent(s),
        }
    }
}
