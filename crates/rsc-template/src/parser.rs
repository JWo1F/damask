use crate::{
    Attr, AttrPart, AttrValue, ClassTerm, EachNode, Element, ElementKind, IfNode, Node, SnippetNode, Span,
    Spanned, Template,
};
use std::fmt;

/// A string slice that remembers its absolute byte offset in the template
/// source. Trimming and prefix-stripping a tag's inner text then yields a
/// precise [`Span`] for the Rust fragment that survives — the offset bookkeeping
/// the language server relies on to map virtual-file positions back to source.
#[derive(Clone, Copy)]
struct Slice<'a> {
    s: &'a str,
    start: usize,
}

impl<'a> Slice<'a> {
    fn new(s: &'a str, start: usize) -> Self {
        Slice { s, start }
    }

    fn trim(self) -> Self {
        let lead = self.s.len() - self.s.trim_start().len();
        Slice {
            s: self.s.trim(),
            start: self.start + lead,
        }
    }

    fn trim_start(self) -> Self {
        let lead = self.s.len() - self.s.trim_start().len();
        Slice {
            s: self.s.trim_start(),
            start: self.start + lead,
        }
    }

    fn strip_prefix(self, p: &str) -> Option<Self> {
        self.s.strip_prefix(p).map(|r| Slice {
            s: r,
            start: self.start + p.len(),
        })
    }

    /// Split on the first `sep`, dropping the separator; both halves keep their
    /// absolute offsets.
    fn split_once(self, sep: &str) -> Option<(Self, Self)> {
        self.s.find(sep).map(|i| {
            (
                Slice::new(&self.s[..i], self.start),
                Slice::new(&self.s[i + sep.len()..], self.start + i + sep.len()),
            )
        })
    }

    fn span(self) -> Span {
        Span::new(self.start, self.start + self.s.len())
    }

    fn to_spanned(self) -> Spanned {
        Spanned::new(self.s, self.span())
    }

    fn is_empty(self) -> bool {
        self.s.is_empty()
    }
}

/// A template parse failure, with the source span it occurred at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (bytes {}..{})",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}

/// HTML void elements — self-closing, no end tag.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Whether `tag` is an HTML void element (rendered without a closing tag).
pub fn is_void_element(tag: &str) -> bool {
    VOID_ELEMENTS.contains(&tag)
}

/// Parse a `.rsc` template into a [`Template`].
pub fn parse(src: &str) -> Result<Template, ParseError> {
    let mut p = Parser::new(src);
    let (nodes, term) = p.parse_nodes()?;
    match term {
        Term::Eof => Ok(Template { nodes }),
        other => Err(p.err_at(p.pos, format!("unexpected {}", other.describe()))),
    }
}

/// What terminated a run of nodes.
enum Term {
    Eof,
    ElementClose(String),
    TagClose(String),
    ElseIf(Spanned),
    Else,
}

impl Term {
    fn describe(&self) -> String {
        match self {
            Term::Eof => "end of input".into(),
            Term::ElementClose(n) => format!("`</{n}>`"),
            Term::TagClose(k) => format!("`{{/{k}}}`"),
            Term::ElseIf(_) => "`{:else if …}`".into(),
            Term::Else => "`{:else}`".into(),
        }
    }
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    n: usize,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            src,
            bytes: src.as_bytes(),
            n: src.len(),
            pos: 0,
        }
    }

    fn err_at(&self, at: usize, message: String) -> ParseError {
        ParseError {
            message,
            span: Span::new(at.min(self.n), self.n),
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        self.src[self.pos..].starts_with(s)
    }

    fn skip_ws(&mut self) {
        while self.pos < self.n && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    /// Parse a run of nodes until a terminator (element/block close, clause, EOF).
    fn parse_nodes(&mut self) -> Result<(Vec<Node>, Term), ParseError> {
        let mut nodes = Vec::new();
        let mut text = String::new();
        // Start offset of the current text run; meaningful only while `text` is
        // non-empty. Set whenever a fresh run begins so `flush!` can span it.
        let mut text_start = self.pos;

        macro_rules! flush {
            () => {
                if !text.is_empty() {
                    let span = Span::new(text_start, self.pos);
                    nodes.push(Node::Text(Spanned::new(std::mem::take(&mut text), span)));
                }
            };
        }

        while self.pos < self.n {
            let b = self.bytes[self.pos];
            if b == b'{' {
                // `{# … #}` — a comment for the reader of the template, which
                // reaches no output at all. Told from `{#if}`/`{#each}` by the
                // whitespace: a block keyword cannot begin with one.
                if self.starts_with("{#") && self.is_comment_open() {
                    flush!();
                    let open = self.pos;
                    let Some(end) = self.src[self.pos + 2..].find("#}") else {
                        return Err(self.err_at(open, "unterminated `{#` comment: missing `#}`".into()));
                    };
                    self.pos += 2 + end + 2;
                    text_start = self.pos;
                    continue;
                }
                flush!();
                match self.parse_tag()? {
                    TagResult::Node(node) => nodes.push(node),
                    TagResult::Term(term) => return Ok((nodes, term)),
                }
            } else if b == b'<' {
                if self.starts_with("<!--") {
                    if text.is_empty() {
                        text_start = self.pos;
                    }
                    let end = self.src[self.pos + 4..]
                        .find("-->")
                        .map(|r| self.pos + 4 + r + 3)
                        .unwrap_or(self.n);
                    text.push_str(&self.src[self.pos..end]);
                    self.pos = end;
                } else if self.starts_with("</") {
                    flush!();
                    let name = self.parse_close_tag()?;
                    return Ok((nodes, Term::ElementClose(name)));
                } else if self.peek_is_element_start() {
                    flush!();
                    let el = self.parse_element()?;
                    nodes.push(Node::Element(el));
                } else {
                    if text.is_empty() {
                        text_start = self.pos;
                    }
                    text.push('<');
                    self.pos += 1;
                }
            } else {
                if text.is_empty() {
                    text_start = self.pos;
                }
                let ch = self.src[self.pos..].chars().next().unwrap();
                text.push(ch);
                self.pos += ch.len_utf8();
            }
        }

        flush!();
        Ok((nodes, Term::Eof))
    }

    fn peek_is_element_start(&self) -> bool {
        self.bytes
            .get(self.pos + 1)
            .is_some_and(|b| b.is_ascii_alphabetic())
    }

    fn parse_close_tag(&mut self) -> Result<String, ParseError> {
        self.pos += 2; // "</"
        let name = self.parse_tag_name()?;
        self.skip_ws();
        if self.pos < self.n && self.bytes[self.pos] == b'>' {
            self.pos += 1;
            Ok(name.text)
        } else {
            Err(self.err_at(
                self.pos,
                format!("expected `>` to close `</{}`", name.as_str()),
            ))
        }
    }

    fn parse_element(&mut self) -> Result<Element, ParseError> {
        let start = self.pos;
        self.pos += 1; // '<'
        let tag = self.parse_tag_name()?;
        let attrs = self.parse_attrs()?;
        self.skip_ws();

        let self_close_syntax = if self.starts_with("/>") {
            self.pos += 2;
            true
        } else if self.pos < self.n && self.bytes[self.pos] == b'>' {
            self.pos += 1;
            false
        } else {
            return Err(self.err_at(start, format!("unclosed `<{}` tag", tag.as_str())));
        };

        let kind = classify_element(tag.as_str());
        let is_void = matches!(kind, ElementKind::Html) && VOID_ELEMENTS.contains(&tag.as_str());

        if self_close_syntax || is_void {
            return Ok(Element {
                tag,
                kind,
                attrs,
                children: Vec::new(),
                self_closing: true,
            });
        }

        let (children, term) = self.parse_nodes()?;
        match term {
            Term::ElementClose(close) if close == *tag.as_str() => Ok(Element {
                tag,
                kind,
                attrs,
                children,
                self_closing: false,
            }),
            Term::ElementClose(other) => Err(self.err_at(
                start,
                format!("`<{}>` closed by `</{other}>`", tag.as_str()),
            )),
            other => Err(self.err_at(
                start,
                format!(
                    "`<{}>` not closed (found {})",
                    tag.as_str(),
                    other.describe()
                ),
            )),
        }
    }

    fn parse_tag_name(&mut self) -> Result<Spanned, ParseError> {
        let start = self.pos;
        while self.pos < self.n {
            let b = self.bytes[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.err_at(start, "expected a tag name".into()));
        }
        Ok(Spanned::new(
            &self.src[start..self.pos],
            Span::new(start, self.pos),
        ))
    }

    fn parse_attrs(&mut self) -> Result<Vec<Attr>, ParseError> {
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            if self.pos >= self.n || self.bytes[self.pos] == b'>' || self.starts_with("/>") {
                break;
            }
            // `{...expr}` stands where an attribute name would, and carries no
            // name of its own — the expression supplies them.
            if self.bytes[self.pos] == b'{' {
                let open = self.pos;
                let inner = self.parse_brace_inner()?;
                let Some(expr) = inner.as_str().strip_prefix("...") else {
                    return Err(self.err_at(
                        open,
                        "expected an attribute name, or `{...expr}` to spread one".into(),
                    ));
                };
                let expr = expr.trim();
                if expr.is_empty() {
                    return Err(self.err_at(open, "empty `{...}` attribute spread".into()));
                }
                // Re-span onto the expression itself, past the `...`.
                let at = inner.span.start + (inner.as_str().len() - expr.len());
                attrs.push(Attr {
                    name: Spanned::new("", Span::new(open, open)),
                    value: AttrValue::Spread(Spanned::new(expr, Span::new(at, at + expr.len()))),
                });
                continue;
            }
            let name = self.parse_attr_name()?;
            self.skip_ws();
            if self.pos < self.n && self.bytes[self.pos] == b'=' {
                self.pos += 1;
                self.skip_ws();
                let is_class = name.as_str() == "class";
                let value = match self.bytes.get(self.pos) {
                    Some(b'"') => AttrValue::Literal(self.parse_quoted(b'"')?),
                    Some(b'\'') => AttrValue::Literal(self.parse_quoted(b'\'')?),
                    Some(b'[') if is_class => AttrValue::Classes(self.parse_class_list()?),
                    Some(b'{') if is_class && self.brace_is_class_map(self.pos) => {
                        let inner = self.parse_brace_inner()?;
                        AttrValue::Classes(parse_class_pairs(&inner)?)
                    }
                    Some(b'{') => AttrValue::Expr(self.parse_brace_inner()?),
                    _ => {
                        return Err(self.err_at(
                            self.pos,
                            format!("expected a value for attribute `{}`", name.as_str()),
                        ));
                    }
                };
                attrs.push(Attr { name, value });
            } else {
                attrs.push(Attr {
                    name,
                    value: AttrValue::Boolean,
                });
            }
        }
        Ok(attrs)
    }

    /// Whether the `{#` at `pos` opens a comment rather than a block tag.
    fn is_comment_open(&self) -> bool {
        self.bytes
            .get(self.pos + 2)
            .is_some_and(|b| b.is_ascii_whitespace())
    }

    fn parse_attr_name(&mut self) -> Result<Spanned, ParseError> {
        let start = self.pos;
        while self.pos < self.n {
            let b = self.bytes[self.pos];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b':' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.err_at(start, "expected an attribute name".into()));
        }
        Ok(Spanned::new(
            &self.src[start..self.pos],
            Span::new(start, self.pos),
        ))
    }

    /// Parse a quoted attribute value into its literal and `{ … }` parts.
    ///
    /// The scan stops at the closing quote, but a `{ … }` group is stepped over
    /// whole by [`Self::scan_braces`] — so a quote inside an interpolation (a
    /// Rust string literal, say, as in `class={if x {"a"} else {"b"}}`) does not
    /// end the attribute.
    fn parse_quoted(&mut self, quote: u8) -> Result<Vec<AttrPart>, ParseError> {
        let start = self.pos;
        self.pos += 1; // opening quote
        let mut parts = Vec::new();
        let mut text_start = self.pos;

        let push_text = |parts: &mut Vec<AttrPart>, from: usize, to: usize, src: &str| {
            if to > from {
                parts.push(AttrPart::Text(Spanned::new(
                    &src[from..to],
                    Span::new(from, to),
                )));
            }
        };

        while self.pos < self.n && self.bytes[self.pos] != quote {
            if self.bytes[self.pos] == b'{' {
                push_text(&mut parts, text_start, self.pos, self.src);
                let open = self.pos;
                let (inner, end) = self.scan_braces(open)?;
                let expr = Slice::new(inner, open + 1).trim().to_spanned();
                if expr.as_str().is_empty() {
                    return Err(self.err_at(open, "empty `{}` in an attribute value".into()));
                }
                parts.push(AttrPart::Expr(expr));
                self.pos = end;
                text_start = self.pos;
            } else {
                self.pos += 1;
            }
        }
        if self.pos >= self.n {
            return Err(self.err_at(start, "unterminated attribute string".into()));
        }
        push_text(&mut parts, text_start, self.pos, self.src);
        self.pos += 1; // closing quote

        // An empty value (`class=""`) still has to survive as a value rather
        // than as no parts at all, or it would be indistinguishable from a
        // boolean attribute downstream.
        if parts.is_empty() {
            parts.push(AttrPart::Text(Spanned::new(
                "",
                Span::new(start + 1, start + 1),
            )));
        }
        Ok(parts)
    }

    /// Whether the `{ … }` at `open` is a class map rather than a Rust block.
    ///
    /// The tell is a top-level `:` that is not part of a `::` path. Rust has no
    /// expression with a bare colon at the top level of a block — type
    /// ascription is not a thing and there is no ternary — so the two forms do
    /// not overlap in practice. A loop label (`'a: loop`) would, but a template
    /// attribute holding one is not a case worth trading this syntax for.
    fn brace_is_class_map(&self, open: usize) -> bool {
        let Ok((inner, _)) = self.scan_braces(open) else {
            return false;
        };
        top_level_colon(inner).is_some()
    }

    /// Parse `[ term, term, … ]` into class terms. `pos` must be at `[`.
    fn parse_class_list(&mut self) -> Result<Vec<ClassTerm>, ParseError> {
        let open = self.pos;
        let inner_start = open + 1;
        let mut i = inner_start;
        let mut depth = 1usize;
        while i < self.n {
            match self.bytes[i] {
                b'"' => i = scan_string(self.src, i),
                b'\'' => i = scan_char(self.src, i),
                b'[' => {
                    depth += 1;
                    i += 1;
                }
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    i += 1;
                }
                _ => i += char_len(self.src, i),
            }
        }
        if i >= self.n {
            return Err(self.err_at(open, "unclosed class list: missing `]`".into()));
        }
        let inner = Slice::new(&self.src[inner_start..i], inner_start);
        self.pos = i + 1;

        let mut terms = Vec::new();
        for item in split_top_level(inner, b',') {
            let item = item.trim();
            if item.s.is_empty() {
                continue;
            }
            // A nested `{ … }` inside a list is a map of conditional classes,
            // so the two forms compose: `[base, { "on": cond }]`.
            if item.s.starts_with('{') && item.s.ends_with('}') {
                let body = Slice::new(&item.s[1..item.s.len() - 1], item.start + 1);
                terms.extend(parse_class_pairs(&body.trim().to_spanned())?);
            } else {
                terms.push(class_term(item.to_spanned()));
            }
        }
        Ok(terms)
    }

    /// Parse a `{ … }` group and return the trimmed inner text with its span.
    /// `pos` must be at `{`.
    fn parse_brace_inner(&mut self) -> Result<Spanned, ParseError> {
        let open = self.pos;
        let (inner, end) = self.scan_braces(open)?;
        self.pos = end;
        Ok(Slice::new(inner, open + 1).trim().to_spanned())
    }

    /// Scan a balanced `{ … }` from `open` (at `{`), skipping string/char
    /// literals. Returns the inner text and the index past the closing `}`.
    fn scan_braces(&self, open: usize) -> Result<(&'a str, usize), ParseError> {
        let inner_start = open + 1;
        let mut i = inner_start;
        let mut depth = 1usize;
        while i < self.n {
            match self.bytes[i] {
                b'"' => i = scan_string(self.src, i),
                b'\'' => i = scan_char(self.src, i),
                b'{' => {
                    depth += 1;
                    i += 1;
                }
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok((&self.src[inner_start..i], i + 1));
                    }
                    i += 1;
                }
                _ => {
                    let ch = self.src[i..].chars().next().unwrap();
                    i += ch.len_utf8();
                }
            }
        }
        Err(ParseError {
            message: "unclosed tag: missing `}`".into(),
            span: Span::new(open, self.n),
        })
    }

    fn parse_tag(&mut self) -> Result<TagResult, ParseError> {
        let open = self.pos;
        let (inner_raw, end) = self.scan_braces(open)?;
        self.pos = end;
        // `inner_raw` is `src[open + 1 .. end - 1]`; carry its absolute start so
        // every fragment we peel off keeps a correct source span.
        let t = Slice::new(inner_raw, open + 1).trim();

        if let Some(body) = t.strip_prefix("@") {
            let body = body.trim_start();
            if let Some(rest) = keyword(body, "html") {
                return Ok(TagResult::Node(Node::Html(rest.to_spanned())));
            }
            if let Some(rest) = keyword(body, "render") {
                return Ok(TagResult::Node(Node::Render(rest.to_spanned())));
            }
            return Err(self.err_at(open, format!("unknown directive `{{@{}}}`", body.s)));
        }

        if let Some(body) = t.strip_prefix("#") {
            let body = body.trim_start();
            if let Some(cond) = keyword(body, "if") {
                return Ok(TagResult::Node(Node::If(
                    self.parse_if(open, cond.to_spanned())?,
                )));
            }
            if let Some(rest) = keyword(body, "each") {
                return Ok(TagResult::Node(Node::Each(self.parse_each(open, rest)?)));
            }
            if let Some(rest) = keyword(body, "snippet") {
                return Ok(TagResult::Node(Node::Snippet(
                    self.parse_snippet(open, rest)?,
                )));
            }
            return Err(self.err_at(open, format!("unknown block `{{#{}}}`", body.s)));
        }

        if let Some(body) = t.strip_prefix(":") {
            let body = body.trim_start();
            if let Some(rest) = keyword(body, "else") {
                let rest = rest.trim_start();
                if let Some(cond) = keyword(rest, "if") {
                    return Ok(TagResult::Term(Term::ElseIf(cond.to_spanned())));
                }
                if rest.is_empty() {
                    return Ok(TagResult::Term(Term::Else));
                }
            }
            return Err(self.err_at(open, format!("unknown clause `{{:{}}}`", body.s)));
        }

        if let Some(body) = t.strip_prefix("/") {
            return Ok(TagResult::Term(Term::TagClose(body.trim().s.to_string())));
        }

        if t.is_empty() {
            return Err(self.err_at(open, "empty tag `{}`".into()));
        }
        // A `{ … }` block — codegen decides value-vs-statement.
        Ok(TagResult::Node(Node::Expr(t.to_spanned())))
    }

    fn parse_if(&mut self, open: usize, first_cond: Spanned) -> Result<IfNode, ParseError> {
        let mut branches = Vec::new();
        let mut cond = first_cond;
        loop {
            let (body, term) = self.parse_nodes()?;
            match term {
                Term::ElseIf(next) => {
                    branches.push((cond, body));
                    cond = next;
                }
                Term::Else => {
                    branches.push((cond, body));
                    let (else_body, t2) = self.parse_nodes()?;
                    return match t2 {
                        Term::TagClose(k) if k == "if" => Ok(IfNode {
                            branches,
                            otherwise: Some(else_body),
                        }),
                        other => Err(self.err_at(
                            open,
                            format!("`{{#if}}` not closed (found {})", other.describe()),
                        )),
                    };
                }
                Term::TagClose(k) if k == "if" => {
                    branches.push((cond, body));
                    return Ok(IfNode {
                        branches,
                        otherwise: None,
                    });
                }
                other => {
                    return Err(self.err_at(
                        open,
                        format!("`{{#if}}` not closed (found {})", other.describe()),
                    ));
                }
            }
        }
    }

    fn parse_each(&mut self, open: usize, code: Slice) -> Result<EachNode, ParseError> {
        let (expr, binding) = code
            .split_once(" as ")
            .ok_or_else(|| self.err_at(open, "`{#each … as …}` requires `as`".into()))?;
        let expr = expr.trim();
        let binding = binding.trim();
        if expr.is_empty() || binding.is_empty() {
            return Err(self.err_at(open, format!("malformed `{{#each {}}}`", code.s)));
        }
        let (body, term) = self.parse_nodes()?;
        match term {
            Term::TagClose(k) if k == "each" => Ok(EachNode {
                expr: expr.to_spanned(),
                binding: binding.to_spanned(),
                body,
            }),
            other => Err(self.err_at(
                open,
                format!("`{{#each}}` not closed (found {})", other.describe()),
            )),
        }
    }

    fn parse_snippet(&mut self, open: usize, code: Slice) -> Result<SnippetNode, ParseError> {
        let paren = code
            .s
            .find('(')
            .ok_or_else(|| self.err_at(open, "`{#snippet name(params)}` needs `(`".into()))?;
        let close = code
            .s
            .rfind(')')
            .ok_or_else(|| self.err_at(open, "`{#snippet name(params)}` needs `)`".into()))?;
        let name = Slice::new(&code.s[..paren], code.start).trim();
        let params = Slice::new(&code.s[paren + 1..close], code.start + paren + 1).trim();
        if name.is_empty() {
            return Err(self.err_at(open, "`{#snippet}` needs a name".into()));
        }
        let (body, term) = self.parse_nodes()?;
        match term {
            Term::TagClose(k) if k == "snippet" => Ok(SnippetNode {
                name: name.to_spanned(),
                params: params.to_spanned(),
                body,
            }),
            other => Err(self.err_at(
                open,
                format!("`{{#snippet}}` not closed (found {})", other.describe()),
            )),
        }
    }
}

enum TagResult {
    Node(Node),
    Term(Term),
}

fn classify_element(tag: &str) -> ElementKind {
    if tag == "slot" {
        ElementKind::Slot
    } else if tag.chars().next().is_some_and(|c| c.is_uppercase()) {
        ElementKind::Component
    } else {
        ElementKind::Html
    }
}

/// If `s` begins with the whole word `k` (not just a prefix of a longer
/// identifier), return what follows it, trimmed. `k` alone (nothing after)
/// yields an empty slice positioned just past `k`.
fn keyword<'a>(s: Slice<'a>, k: &str) -> Option<Slice<'a>> {
    let rest = s.strip_prefix(k)?;
    match rest.s.chars().next() {
        None => Some(Slice::new("", rest.start)),
        Some(c) if c.is_alphanumeric() || c == '_' => None,
        Some(_) => Some(rest.trim()),
    }
}

fn scan_string(src: &str, i: usize) -> usize {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut j = i + 1;
    while j < n {
        match bytes[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => {
                let ch = src[j..].chars().next().unwrap();
                j += ch.len_utf8();
            }
        }
    }
    n
}

fn scan_char(src: &str, i: usize) -> usize {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut j = i + 1;
    if j < n && bytes[j] == b'\\' {
        j += 2;
        if j < n && bytes[j] == b'\'' {
            return j + 1;
        }
    } else if j < n {
        let ch = src[j..].chars().next().unwrap();
        let after = j + ch.len_utf8();
        if after < n && bytes[after] == b'\'' {
            return after + 1;
        }
    }
    i + 1
}

/// Byte ranges of every `{ … }` tag in `src` — the code-bearing regions to
/// blank out when projecting a template to plain HTML for an HTML language
/// server. Covers both text-position tags and attribute expression values
/// (`attr={…}`), and skips HTML comments and quoted attribute strings so the
/// braces inside them are left as literal text.
///
/// This lives beside the parser so it uses the same tokenization rules the real
/// parser does, and can't drift from them.
pub fn tag_spans(src: &str) -> Vec<Span> {
    let bytes = src.as_bytes();
    let n = src.len();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < n {
        match bytes[i] {
            b'<' if src[i..].starts_with("<!--") => {
                i = src[i + 4..].find("-->").map(|r| i + 4 + r + 3).unwrap_or(n);
            }
            b'<' => {
                // Inside an element tag: keep the structure, record `{…}`
                // attribute values, and skip quoted strings, up to `>`.
                i += 1;
                while i < n && bytes[i] != b'>' {
                    match bytes[i] {
                        b'"' => i = scan_to_quote(src, i, b'"'),
                        b'\'' => i = scan_to_quote(src, i, b'\''),
                        b'{' => {
                            let end = scan_braces_end(src, i);
                            spans.push(Span::new(i, end));
                            i = end;
                        }
                        _ => i += char_len(src, i),
                    }
                }
                if i < n {
                    i += 1; // consume '>'
                }
            }
            b'{' => {
                let end = scan_braces_end(src, i);
                spans.push(Span::new(i, end));
                i = end;
            }
            _ => i += char_len(src, i),
        }
    }
    spans
}

/// Byte length of the UTF-8 char at `i` (at least 1).
fn char_len(src: &str, i: usize) -> usize {
    src[i..].chars().next().map(char::len_utf8).unwrap_or(1)
}

/// From an opening quote at `open`, return the index just past the closing
/// `quote` (or end of input). RSC attribute strings have no escapes.
fn scan_to_quote(src: &str, open: usize, quote: u8) -> usize {
    let bytes = src.as_bytes();
    let n = src.len();
    let mut i = open + 1;
    while i < n && bytes[i] != quote {
        i += char_len(src, i);
    }
    (i + 1).min(n)
}

/// From `{` at `open`, return the index just past the matching `}` (or end of
/// input), skipping string and char literals inside.
/// A single class-list term: a literal `None` drops out, anything else is Rust.
fn class_term(text: Spanned) -> ClassTerm {
    if text.as_str() == "None" {
        ClassTerm::Nothing
    } else {
        ClassTerm::Expr(text)
    }
}

/// Byte offset of the first top-level `:` that is not part of `::`, if any.
///
/// "Top level" means outside every bracket, string and char literal, so the
/// colon in `{ "a": matches!(x, Foo::B) }` is found and the one in `Foo::B` is
/// not.
fn top_level_colon(src: &str) -> Option<usize> {
    let bytes = src.as_bytes();
    let n = src.len();
    let mut i = 0usize;
    let mut depth = 0i32;
    while i < n {
        match bytes[i] {
            b'"' => i = scan_string(src, i),
            b'\'' => i = scan_char(src, i),
            b'(' | b'[' | b'{' | b'<' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' | b'}' | b'>' => {
                depth -= 1;
                i += 1;
            }
            b':' if depth == 0 => {
                if bytes.get(i + 1) == Some(&b':') {
                    i += 2;
                } else {
                    return Some(i);
                }
            }
            _ => i += char_len(src, i),
        }
    }
    None
}

/// Split on a top-level separator, keeping each piece's absolute span.
fn split_top_level(src: Slice<'_>, sep: u8) -> Vec<Slice<'_>> {
    let bytes = src.s.as_bytes();
    let n = src.s.len();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut start = 0usize;
    let mut depth = 0i32;
    while i < n {
        match bytes[i] {
            b'"' => i = scan_string(src.s, i),
            b'\'' => i = scan_char(src.s, i),
            b'(' | b'[' | b'{' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                i += 1;
            }
            b if b == sep && depth == 0 => {
                out.push(Slice::new(&src.s[start..i], src.start + start));
                i += 1;
                start = i;
            }
            _ => i += char_len(src.s, i),
        }
    }
    out.push(Slice::new(&src.s[start..n], src.start + start));
    out
}

/// Parse `"a": cond, "b": cond` into conditional class terms.
fn parse_class_pairs(body: &Spanned) -> Result<Vec<ClassTerm>, ParseError> {
    let src = Slice::new(body.as_str(), body.span.start);
    let mut terms = Vec::new();
    for pair in split_top_level(src, b',') {
        let pair = pair.trim();
        if pair.s.is_empty() {
            continue;
        }
        let Some(at) = top_level_colon(pair.s) else {
            return Err(ParseError {
                message: format!("expected `name: condition` in a class map, found `{}`", pair.s),
                span: Span::new(pair.start, pair.start + pair.s.len()),
            });
        };
        let name = Slice::new(&pair.s[..at], pair.start).trim();
        let when = Slice::new(&pair.s[at + 1..], pair.start + at + 1).trim();
        if name.s.is_empty() || when.s.is_empty() {
            return Err(ParseError {
                message: "a class map entry needs both a name and a condition".into(),
                span: Span::new(pair.start, pair.start + pair.s.len()),
            });
        }
        terms.push(ClassTerm::Cond {
            name: name.to_spanned(),
            when: when.to_spanned(),
        });
    }
    Ok(terms)
}

fn scan_braces_end(src: &str, open: usize) -> usize {
    let bytes = src.as_bytes();
    let n = src.len();
    let mut i = open + 1;
    let mut depth = 1usize;
    while i < n {
        match bytes[i] {
            b'"' => i = scan_string(src, i),
            b'\'' => i = scan_char(src, i),
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => i += char_len(src, i),
        }
    }
    n
}

/// Whether `offset` lies inside an open `{ … }` tag (brace depth > 0), tolerant
/// of half-typed tags. Used by the language server for completion context.
pub fn in_tag(src: &str, offset: usize) -> bool {
    let offset = offset.min(src.len());
    let bytes = src.as_bytes();
    let mut i = 0usize;
    let mut depth: i32 = 0;
    while i < offset {
        if bytes[i] == b'<' && src[i..].starts_with("<!--") {
            let end = src[i + 4..]
                .find("-->")
                .map(|r| i + 4 + r + 3)
                .unwrap_or(src.len());
            if end > offset {
                return false;
            }
            i = end;
            continue;
        }
        match bytes[i] {
            b'"' if depth > 0 => i = scan_string(src, i),
            b'\'' if depth > 0 => i = scan_char(src, i),
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = (depth - 1).max(0);
                i += 1;
            }
            _ => {
                let ch = src[i..].chars().next().unwrap();
                i += ch.len_utf8();
            }
        }
    }
    depth > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes(src: &str) -> Vec<Node> {
        parse(src).unwrap().nodes
    }

    #[test]
    fn text_and_expr() {
        assert_eq!(
            nodes("Hi {self.name}!"),
            vec![
                Node::Text("Hi ".into()),
                Node::Expr("self.name".into()),
                Node::Text("!".into())
            ]
        );
        assert_eq!(nodes("{2 + 3; 10}"), vec![Node::Expr("2 + 3; 10".into())]);
        assert_eq!(nodes("{let a = 1}"), vec![Node::Expr("let a = 1".into())]);
    }

    #[test]
    fn directives_and_use() {
        assert_eq!(nodes("{@html x}"), vec![Node::Html("x".into())]);
        assert_eq!(nodes("{@render foo}"), vec![Node::Render("foo".into())]);
        // `use` is a plain block statement, not a special tag.
        assert_eq!(
            nodes("{use crate::Card}"),
            vec![Node::Expr("use crate::Card".into())]
        );
    }

    #[test]
    fn if_block() {
        let n = nodes("{#if a}A{:else if b}B{:else}C{/if}");
        match &n[0] {
            Node::If(i) => {
                assert_eq!(i.branches.len(), 2);
                assert_eq!(i.branches[0].0, "a");
                assert_eq!(i.branches[1].0, "b");
                assert!(i.otherwise.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn each_block() {
        let n = nodes("{#each &self.items as item}<li>{item}</li>{/each}");
        match &n[0] {
            Node::Each(e) => {
                assert_eq!(e.expr, "&self.items");
                assert_eq!(e.binding, "item");
                assert_eq!(e.body.len(), 1); // one <li> element
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn html_element_with_attrs() {
        let n = nodes(r#"<a href={self.url} class="link" download>go</a>"#);
        match &n[0] {
            Node::Element(el) => {
                assert_eq!(el.tag, "a");
                assert_eq!(el.kind, ElementKind::Html);
                assert_eq!(el.attrs.len(), 3);
                assert_eq!(
                    el.attrs[0],
                    Attr {
                        name: "href".into(),
                        value: AttrValue::Expr("self.url".into())
                    }
                );
                assert_eq!(
                    el.attrs[1],
                    Attr {
                        name: "class".into(),
                        value: AttrValue::text("link")
                    }
                );
                assert_eq!(
                    el.attrs[2],
                    Attr {
                        name: "download".into(),
                        value: AttrValue::Boolean
                    }
                );
                assert_eq!(el.children, vec![Node::Text("go".into())]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn void_and_self_closing() {
        match &nodes("<br>")[0] {
            Node::Element(el) => assert!(el.self_closing && el.children.is_empty()),
            other => panic!("{other:?}"),
        }
        match &nodes("<Foo x={1}/>")[0] {
            Node::Element(el) => {
                assert_eq!(el.kind, ElementKind::Component);
                assert!(el.self_closing);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn component_with_slots() {
        let n = nodes(r#"<Card title={2 + 8}>default<slot name="foot">f</slot></Card>"#);
        match &n[0] {
            Node::Element(el) => {
                assert_eq!(el.kind, ElementKind::Component);
                assert_eq!(
                    el.attrs[0],
                    Attr {
                        name: "title".into(),
                        value: AttrValue::Expr("2 + 8".into())
                    }
                );
                // children: text "default", then a <slot name="foot"> element
                assert_eq!(el.children.len(), 2);
                match &el.children[1] {
                    Node::Element(slot) => {
                        assert_eq!(slot.kind, ElementKind::Slot);
                        assert_eq!(
                            slot.attrs[0],
                            Attr {
                                name: "name".into(),
                                value: AttrValue::text("foot")
                            }
                        );
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nested_braces_in_attr_and_render() {
        let n = nodes(r#"<Foo data={Bar { x: "}".into() }}/>"#);
        match &n[0] {
            Node::Element(el) => {
                assert_eq!(
                    el.attrs[0].value,
                    AttrValue::Expr(r#"Bar { x: "}".into() }"#.into())
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn mismatched_close_errors() {
        assert!(parse("<div>{#if x}</div>{/if}").is_err());
        assert!(
            parse("<div>hi</span>")
                .unwrap_err()
                .message
                .contains("closed by")
        );
    }

    #[test]
    fn html_comment_is_text() {
        assert_eq!(
            nodes("a<!-- {x} -->b"),
            vec![Node::Text("a<!-- {x} -->b".into())]
        );
    }

    #[test]
    fn in_tag_detection() {
        assert!(in_tag("Hi { self.na", 12));
        assert!(!in_tag("Hi { x } bye", 12));
        assert!(in_tag("<Foo a={Bar { ", 14));
    }

    /// Blanking the reported spans (length-preserving) leaves valid HTML markup
    /// and removes every `{ … }` region — including attribute expression values
    /// and control-flow markers.
    #[test]
    fn tag_spans_cover_code_regions_only() {
        let src = r#"<a href={self.url}>Hi {self.name}!</a>{#if x}y{/if}"#;
        let mut blanked = src.to_string();
        for s in tag_spans(src) {
            blanked.replace_range(s.start..s.end, &" ".repeat(s.len()));
        }
        assert!(!blanked.contains('{'), "unblanked tag remains: {blanked:?}");
        assert!(!blanked.contains('}'));
        // The HTML structure survives, so an HTML server sees real markup.
        assert!(blanked.contains("<a href="));
        assert!(blanked.contains("Hi "));
        assert!(blanked.contains("</a>"));
        assert_eq!(
            blanked.len(),
            src.len(),
            "length preserved for identity mapping"
        );
    }

    #[test]
    fn comments_reach_no_output() {
        let t = parse("a{# not rendered #}b").unwrap();
        assert_eq!(t.nodes, vec![Node::Text("a".into()), Node::Text("b".into())]);
        // A block keyword cannot begin with whitespace, so `{#if}` is untouched.
        assert!(matches!(
            parse("{#if x}y{/if}").unwrap().nodes.as_slice(),
            [Node::If(_)]
        ));
        // Braces and tags inside a comment are text, not structure.
        assert_eq!(parse("{#\n {self.x} <div> \n#}").unwrap().nodes, vec![]);
        assert!(parse("{# unterminated").is_err());
    }

    #[test]
    fn tag_spans_preserve_quoted_attribute_braces() {
        // A `{` inside a quoted attribute literal is text, not a tag.
        let src = r#"<a title="{x}">t</a>"#;
        assert!(tag_spans(src).is_empty(), "quoted braces must not be tags");
    }

    /// Collect every span-carrying fragment in a node tree, in source order.
    fn collect<'a>(nodes: &'a [Node], out: &mut Vec<&'a Spanned>) {
        for n in nodes {
            match n {
                Node::Text(s) | Node::Expr(s) | Node::Html(s) | Node::Render(s) => out.push(s),
                Node::If(i) => {
                    for (cond, body) in &i.branches {
                        out.push(cond);
                        collect(body, out);
                    }
                    if let Some(body) = &i.otherwise {
                        collect(body, out);
                    }
                }
                Node::Each(e) => {
                    out.push(&e.expr);
                    out.push(&e.binding);
                    collect(&e.body, out);
                }
                Node::Snippet(s) => {
                    out.push(&s.name);
                    out.push(&s.params);
                    collect(&s.body, out);
                }
                Node::Element(el) => {
                    for a in &el.attrs {
                        match &a.value {
                            AttrValue::Expr(v) | AttrValue::Spread(v) => out.push(v),
                            AttrValue::Literal(parts) => {
                                for part in parts {
                                    match part {
                                        AttrPart::Text(t) | AttrPart::Expr(t) => out.push(t),
                                    }
                                }
                            }
                            AttrValue::Classes(terms) => {
                                for term in terms {
                                    match term {
                                        ClassTerm::Expr(t) => out.push(t),
                                        ClassTerm::Cond { name, when } => {
                                            out.push(name);
                                            out.push(when);
                                        }
                                        ClassTerm::Nothing => {}
                                    }
                                }
                            }
                            AttrValue::Boolean => {}
                        }
                    }
                    collect(&el.children, out);
                }
            }
        }
    }

    /// The core span invariant: slicing the source with a fragment's span yields
    /// exactly that fragment's text. This is what makes the spans usable as a
    /// virtual-file ↔ source position map.
    #[test]
    fn spans_slice_back_to_source() {
        let src = concat!(
            r#"Hi {self.name}! <a href={self.url} title="go">{@html self.body}</a>"#,
            "{#if self.ok}yes{:else if self.maybe}m{:else}no{/if}",
            "{#each &self.items as item, i}{item}{/each}",
            "{#snippet foo(x: u8)}z{/snippet}",
        );
        let t = parse(src).unwrap();
        let mut frags = Vec::new();
        collect(&t.nodes, &mut frags);
        assert!(
            frags.len() > 10,
            "expected many fragments, got {}",
            frags.len()
        );
        for f in &frags {
            assert_eq!(
                f.span.slice(src),
                f.text,
                "span {:?} does not slice back to {:?}",
                (f.span.start, f.span.end),
                f.text,
            );
        }
    }

    #[test]
    fn concrete_span_offsets() {
        // "Hi {self.name}!" — text "Hi " is 0..3, the expr "self.name" is 4..13.
        let n = parse("Hi {self.name}!").unwrap().nodes;
        match &n[0] {
            Node::Text(s) => assert_eq!((s.span.start, s.span.end), (0, 3)),
            other => panic!("{other:?}"),
        }
        match &n[1] {
            Node::Expr(s) => {
                assert_eq!(s.text, "self.name");
                assert_eq!((s.span.start, s.span.end), (4, 13));
            }
            other => panic!("{other:?}"),
        }
    }
}
