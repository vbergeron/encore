//! Minimal S-expression reader and pretty-printer shared by the IR text formats.
//!
//! Lexical syntax:
//! - `; ...` line comments
//! - integers: `42`, `-7`
//! - strings (byte literals): `"a\n\x00\"\\"`
//! - symbols: any run of chars other than whitespace, `(`, `)`, `"`, `;`, `|`
//!   that is not an integer; arbitrary symbols are written `|...|` with
//!   `\|` and `\\` escapes
//! - lists: `( ... )`

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Sexp {
    Sym(String),
    Int(i64),
    Str(Vec<u8>),
    List(Vec<Sexp>),
}

/// A parse error with a 1-based source line.
#[derive(Debug, Clone, PartialEq)]
pub struct TextError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TextError {}

/// A parsed node together with the line it starts on.
#[derive(Debug, Clone)]
pub struct Node {
    pub line: usize,
    pub kind: NodeKind,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Sym(String),
    Int(i64),
    Str(Vec<u8>),
    List(Vec<Node>),
}

impl Node {
    pub fn err<T>(&self, message: impl Into<String>) -> Result<T, TextError> {
        Err(TextError { line: self.line, message: message.into() })
    }

    pub fn describe(&self) -> String {
        match &self.kind {
            NodeKind::Sym(s) => format!("symbol `{s}`"),
            NodeKind::Int(i) => format!("integer {i}"),
            NodeKind::Str(_) => "string".into(),
            NodeKind::List(items) => match items.first().map(|n| &n.kind) {
                Some(NodeKind::Sym(h)) => format!("`({h} ...)`"),
                _ => "list".into(),
            },
        }
    }

    pub fn as_sym(&self) -> Result<&str, TextError> {
        match &self.kind {
            NodeKind::Sym(s) => Ok(s),
            _ => self.err(format!("expected a symbol, found {}", self.describe())),
        }
    }

    pub fn as_int(&self) -> Result<i64, TextError> {
        match &self.kind {
            NodeKind::Int(i) => Ok(*i),
            _ => self.err(format!("expected an integer, found {}", self.describe())),
        }
    }

    pub fn as_str(&self) -> Result<&[u8], TextError> {
        match &self.kind {
            NodeKind::Str(s) => Ok(s),
            _ => self.err(format!("expected a string, found {}", self.describe())),
        }
    }

    pub fn as_list(&self) -> Result<&[Node], TextError> {
        match &self.kind {
            NodeKind::List(items) => Ok(items),
            _ => self.err(format!("expected a list, found {}", self.describe())),
        }
    }

    /// Integer that must fit in `T` (e.g. `u8` tags, `u16` slots).
    pub fn as_num<T: TryFrom<i64>>(&self, what: &str) -> Result<T, TextError> {
        let i = self.as_int()?;
        T::try_from(i).or_else(|_| self.err(format!("{what} out of range: {i}")))
    }

    /// If this is a list headed by a symbol, return `(head, rest)`.
    pub fn as_form(&self) -> Option<(&str, &[Node])> {
        match &self.kind {
            NodeKind::List(items) => match items.split_first() {
                Some((Node { kind: NodeKind::Sym(h), .. }, rest)) => Some((h.as_str(), rest)),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn sym_list(&self) -> Result<Vec<String>, TextError> {
        self.as_list()?.iter().map(|n| n.as_sym().map(str::to_string)).collect()
    }
}

/// Check the argument count of a form.
pub fn arity(node: &Node, head: &str, args: &[Node], n: usize) -> Result<(), TextError> {
    if args.len() != n {
        return node.err(format!("`{head}` expects {n} argument(s), found {}", args.len()));
    }
    Ok(())
}

pub fn arity_at_least(node: &Node, head: &str, args: &[Node], n: usize) -> Result<(), TextError> {
    if args.len() < n {
        return node.err(format!("`{head}` expects at least {n} argument(s), found {}", args.len()));
    }
    Ok(())
}

// ── Reader ──────────────────────────────────────────────────────────────────

struct Reader<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
}

pub fn parse(src: &str) -> Result<Vec<Node>, TextError> {
    let mut r = Reader { src: src.as_bytes(), pos: 0, line: 1 };
    let mut out = Vec::new();
    loop {
        r.skip_ws();
        if r.pos >= r.src.len() {
            return Ok(out);
        }
        out.push(r.node()?);
    }
}

fn is_delim(c: u8) -> bool {
    c.is_ascii_whitespace() || matches!(c, b'(' | b')' | b'"' | b';' | b'|')
}

impl<'a> Reader<'a> {
    fn err<T>(&self, message: impl Into<String>) -> Result<T, TextError> {
        Err(TextError { line: self.line, message: message.into() })
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek()?;
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
        }
        Some(c)
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.bump();
            } else if c == b';' {
                while let Some(c) = self.peek() {
                    if c == b'\n' { break; }
                    self.bump();
                }
            } else {
                break;
            }
        }
    }

    fn node(&mut self) -> Result<Node, TextError> {
        let line = self.line;
        let kind = match self.peek() {
            Some(b'(') => {
                self.bump();
                let mut items = Vec::new();
                loop {
                    self.skip_ws();
                    match self.peek() {
                        None => return Err(TextError { line, message: "unclosed `(`".into() }),
                        Some(b')') => { self.bump(); break; }
                        Some(_) => items.push(self.node()?),
                    }
                }
                NodeKind::List(items)
            }
            Some(b')') => return self.err("unexpected `)`"),
            Some(b'"') => {
                self.bump();
                NodeKind::Str(self.escaped(b'"')?)
            }
            Some(b'|') => {
                self.bump();
                let bytes = self.escaped(b'|')?;
                match String::from_utf8(bytes) {
                    Ok(s) => NodeKind::Sym(s),
                    Err(_) => return self.err("symbol is not valid UTF-8"),
                }
            }
            Some(_) => {
                let start = self.pos;
                while self.peek().is_some_and(|c| !is_delim(c)) {
                    self.bump();
                }
                let tok = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
                if looks_numeric(tok) {
                    match tok.parse::<i64>() {
                        Ok(i) => NodeKind::Int(i),
                        Err(_) => return self.err(format!("invalid integer `{tok}`")),
                    }
                } else {
                    NodeKind::Sym(tok.to_string())
                }
            }
            None => return self.err("unexpected end of input"),
        };
        Ok(Node { line, kind })
    }

    fn escaped(&mut self, close: u8) -> Result<Vec<u8>, TextError> {
        let mut out = Vec::new();
        loop {
            match self.bump() {
                None => return self.err(format!("unterminated `{}`", close as char)),
                Some(c) if c == close => return Ok(out),
                Some(b'\\') => match self.bump() {
                    Some(b'n') => out.push(b'\n'),
                    Some(b't') => out.push(b'\t'),
                    Some(b'r') => out.push(b'\r'),
                    Some(b'x') => {
                        let hex = |r: &mut Self| -> Result<u8, TextError> {
                            match r.bump().and_then(|c| (c as char).to_digit(16)) {
                                Some(d) => Ok(d as u8),
                                None => r.err("expected two hex digits after `\\x`"),
                            }
                        };
                        let hi = hex(self)?;
                        let lo = hex(self)?;
                        out.push(hi << 4 | lo);
                    }
                    Some(c @ (b'\\' | b'"' | b'|')) => out.push(c),
                    Some(c) => return self.err(format!("unknown escape `\\{}`", c as char)),
                    None => return self.err("unterminated escape"),
                },
                Some(c) => out.push(c),
            }
        }
    }
}

/// Tokens starting like a number are read as integers (and must be valid).
fn looks_numeric(tok: &str) -> bool {
    let b = tok.as_bytes();
    match b {
        [c, ..] if c.is_ascii_digit() => true,
        [b'-' | b'+', c, ..] if c.is_ascii_digit() => true,
        _ => false,
    }
}

// ── Printer ─────────────────────────────────────────────────────────────────

const WIDTH: usize = 80;
/// Lists up to this width may stay on their parent's opening line.
const INLINE_ARG_WIDTH: usize = 32;

pub fn sym(s: impl Into<String>) -> Sexp {
    Sexp::Sym(s.into())
}

pub fn int(i: impl Into<i64>) -> Sexp {
    Sexp::Int(i.into())
}

pub fn list(items: impl IntoIterator<Item = Sexp>) -> Sexp {
    Sexp::List(items.into_iter().collect())
}

pub fn form(head: &str, args: impl IntoIterator<Item = Sexp>) -> Sexp {
    let mut items = vec![sym(head)];
    items.extend(args);
    Sexp::List(items)
}

pub fn sym_list<S: AsRef<str>>(names: &[S]) -> Sexp {
    list(names.iter().map(|n| sym(n.as_ref())))
}

fn write_sym(s: &str, out: &mut String) {
    let plain = !s.is_empty() && !looks_numeric(s) && s.bytes().all(|c| !is_delim(c) && c.is_ascii_graphic());
    if plain {
        out.push_str(s);
    } else {
        out.push('|');
        for c in s.chars() {
            match c {
                '|' => out.push_str("\\|"),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                c => out.push(c),
            }
        }
        out.push('|');
    }
}

fn write_str(bytes: &[u8], out: &mut String) {
    out.push('"');
    for &b in bytes {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\t' => out.push_str("\\t"),
            b'\r' => out.push_str("\\r"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out.push('"');
}

fn write_flat(e: &Sexp, out: &mut String) {
    match e {
        Sexp::Sym(s) => write_sym(s, out),
        Sexp::Int(i) => out.push_str(&i.to_string()),
        Sexp::Str(b) => write_str(b, out),
        Sexp::List(items) => {
            out.push('(');
            for (i, item) in items.iter().enumerate() {
                if i > 0 { out.push(' '); }
                write_flat(item, out);
            }
            out.push(')');
        }
    }
}

/// Flat width of `e`, or `None` once it exceeds `budget` (keeps layout linear).
fn flat_width(e: &Sexp, budget: usize) -> Option<usize> {
    let w = match e {
        Sexp::Sym(_) | Sexp::Int(_) | Sexp::Str(_) => {
            let mut s = String::new();
            write_flat(e, &mut s);
            s.len()
        }
        Sexp::List(items) => {
            let mut w = 2 + items.len().saturating_sub(1);
            for item in items {
                w += flat_width(item, budget.checked_sub(w)?)?;
            }
            w
        }
    };
    (w <= budget).then_some(w)
}

/// A list too wide for one line keeps its head and leading atoms / short
/// lists on the opening line (never the last item); the remaining items are broken onto
/// indented lines once the flat form exceeds the width.
fn write_pretty(e: &Sexp, indent: usize, out: &mut String) {
    let items = match e {
        Sexp::List(items) if !items.is_empty() && flat_width(e, WIDTH.saturating_sub(indent)).is_none() => items,
        _ => { write_flat(e, out); return; }
    };
    out.push('(');
    // Keep atoms and binder lists after the head on the opening line.
    let mut split = 1;
    let mut col = indent + 1 + flat_width(&items[0], usize::MAX).unwrap();
    while split < items.len() && split + 1 < items.len() {
        let budget = if matches!(items[split], Sexp::List(_)) { INLINE_ARG_WIDTH } else { usize::MAX };
        let Some(w) = flat_width(&items[split], budget) else { break };
        if col + 1 + w > WIDTH {
            break;
        }
        col += 1 + w;
        split += 1;
    }
    write_pretty(&items[0], indent + 1, out);
    for item in &items[1..split] {
        out.push(' ');
        write_flat(item, out);
    }
    for item in &items[split..] {
        out.push('\n');
        for _ in 0..indent + 2 { out.push(' '); }
        write_pretty(item, indent + 2, out);
    }
    out.push(')');
}

pub fn pretty(e: &Sexp) -> String {
    let mut s = String::new();
    write_pretty(e, 0, &mut s);
    s
}

/// Render top-level forms separated by blank lines, with a leading header comment.
pub fn render_module(header: &str, forms: &[Sexp]) -> String {
    let mut s = format!("; {header}\n");
    for f in forms {
        s.push('\n');
        s.push_str(&pretty(f));
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_round_trip() {
        for name in ["x", "_f0", "Nat.add", "a'", "", "has space", "1x", "-3", "|", "a\\b", "(p)"] {
            let mut s = String::new();
            write_sym(name, &mut s);
            let nodes = parse(&s).unwrap();
            assert_eq!(nodes.len(), 1, "{s}");
            assert_eq!(nodes[0].as_sym().unwrap(), name, "{s}");
        }
    }

    #[test]
    fn strings_round_trip() {
        let bytes: Vec<u8> = (0..=255).collect();
        let mut s = String::new();
        write_str(&bytes, &mut s);
        let nodes = parse(&s).unwrap();
        assert_eq!(nodes[0].as_str().unwrap(), &bytes[..]);
    }

    #[test]
    fn comments_and_lines() {
        let nodes = parse("; hi\n(a 1\n  ; c\n  -2) b").unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[1].line, 4);
        let err = parse("(a\n(b)").unwrap_err();
        assert_eq!(err.line, 1);
    }
}
