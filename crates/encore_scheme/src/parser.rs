#[derive(Debug, Clone, PartialEq)]
pub enum Sexp {
    Atom(String),
    List(Vec<Sexp>),
}

impl Sexp {
    pub fn atom(s: &str) -> Sexp {
        Sexp::Atom(s.to_string())
    }

    pub fn list(items: Vec<Sexp>) -> Sexp {
        Sexp::List(items)
    }

    pub fn as_atom(&self) -> Option<&str> {
        match self {
            Sexp::Atom(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Sexp]> {
        match self {
            Sexp::List(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub msg: String,
    pub pos: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at byte {}: {}", self.pos, self.msg)
    }
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let c = self.input.get(self.pos).copied()?;
        self.pos += 1;
        Some(c)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r') => {
                    self.pos += 1;
                }
                Some(b';') => {
                    while let Some(c) = self.peek() {
                        self.pos += 1;
                        if c == b'\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
    }

    fn err(&self, msg: &str) -> ParseError {
        ParseError {
            msg: msg.to_string(),
            pos: self.pos,
        }
    }

    fn is_atom_char(c: u8) -> bool {
        !matches!(
            c,
            b'(' | b')' | b'\'' | b'`' | b',' | b'"' | b';' | b' ' | b'\t' | b'\n' | b'\r'
        )
    }

    fn parse_atom(&mut self) -> Result<Sexp, ParseError> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if Self::is_atom_char(c) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.err("expected atom"));
        }
        let text = std::str::from_utf8(&self.input[start..self.pos]).unwrap();
        Ok(Sexp::Atom(text.to_string()))
    }

    fn parse_list(&mut self) -> Result<Sexp, ParseError> {
        assert_eq!(self.advance(), Some(b'('));
        let mut items = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            match self.peek() {
                None => return Err(self.err("unexpected EOF in list")),
                Some(b')') => {
                    self.pos += 1;
                    return Ok(Sexp::List(items));
                }
                _ => items.push(self.parse_sexp()?),
            }
        }
    }

    fn parse_sexp(&mut self) -> Result<Sexp, ParseError> {
        self.skip_whitespace_and_comments();
        match self.peek() {
            None => Err(self.err("unexpected EOF")),
            Some(b'(') => self.parse_list(),
            Some(b'\'') => {
                self.pos += 1;
                let inner = self.parse_sexp()?;
                Ok(Sexp::List(vec![Sexp::atom("quote"), inner]))
            }
            Some(b'`') => {
                self.pos += 1;
                let inner = self.parse_sexp()?;
                Ok(Sexp::List(vec![Sexp::atom("quasiquote"), inner]))
            }
            Some(b',') => {
                self.pos += 1;
                let inner = self.parse_sexp()?;
                Ok(Sexp::List(vec![Sexp::atom("unquote"), inner]))
            }
            Some(b'"') => self.parse_string(),
            Some(b')') => Err(self.err("unexpected ')'")),
            _ => self.parse_atom(),
        }
    }

    fn parse_string(&mut self) -> Result<Sexp, ParseError> {
        assert_eq!(self.advance(), Some(b'"'));
        let start = self.pos;
        while let Some(c) = self.advance() {
            if c == b'"' {
                let text = std::str::from_utf8(&self.input[start..self.pos - 1]).unwrap();
                return Ok(Sexp::Atom(format!("\"{text}\"")));
            }
        }
        Err(self.err("unterminated string"))
    }
}

pub fn parse(input: &str) -> Result<Vec<Sexp>, ParseError> {
    let mut p = Parser::new(input);
    let mut results = Vec::new();
    loop {
        p.skip_whitespace_and_comments();
        if p.peek().is_none() {
            break;
        }
        results.push(p.parse_sexp()?);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_atom_test() {
        assert_eq!(parse("hello").unwrap(), vec![Sexp::atom("hello")]);
    }

    #[test]
    fn parse_list_test() {
        let result = parse("(a b c)").unwrap();
        assert_eq!(
            result,
            vec![Sexp::list(vec![
                Sexp::atom("a"),
                Sexp::atom("b"),
                Sexp::atom("c"),
            ])]
        );
    }

    #[test]
    fn parse_nested() {
        let result = parse("(define (f x) (+ x 1))").unwrap();
        assert_eq!(
            result,
            vec![Sexp::list(vec![
                Sexp::atom("define"),
                Sexp::list(vec![Sexp::atom("f"), Sexp::atom("x")]),
                Sexp::list(vec![Sexp::atom("+"), Sexp::atom("x"), Sexp::atom("1")]),
            ])]
        );
    }

    #[test]
    fn parse_quasiquote() {
        let result = parse("`(Cons ,x ,y)").unwrap();
        assert_eq!(
            result,
            vec![Sexp::list(vec![
                Sexp::atom("quasiquote"),
                Sexp::list(vec![
                    Sexp::atom("Cons"),
                    Sexp::list(vec![Sexp::atom("unquote"), Sexp::atom("x")]),
                    Sexp::list(vec![Sexp::atom("unquote"), Sexp::atom("y")]),
                ]),
            ])]
        );
    }

    #[test]
    fn parse_comments() {
        let result = parse(";; comment\n(a b)").unwrap();
        assert_eq!(
            result,
            vec![Sexp::list(vec![Sexp::atom("a"), Sexp::atom("b")])]
        );
    }

    #[test]
    fn parse_quote() {
        let result = parse("'foo").unwrap();
        assert_eq!(
            result,
            vec![Sexp::list(vec![Sexp::atom("quote"), Sexp::atom("foo")])]
        );
    }
}
