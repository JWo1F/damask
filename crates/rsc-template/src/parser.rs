use crate::{Attr, AttrValue, EachNode, Element, ElementKind, IfNode, Node, SnippetNode, Span, Template};
use std::fmt;

/// A template parse failure, with the source span it occurred at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (bytes {}..{})", self.message, self.span.start, self.span.end)
    }
}

impl std::error::Error for ParseError {}

/// HTML void elements — self-closing, no end tag.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta",
    "param", "source", "track", "wbr",
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
    ElseIf(String),
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
        Parser { src, bytes: src.as_bytes(), n: src.len(), pos: 0 }
    }

    fn err_at(&self, at: usize, message: String) -> ParseError {
        ParseError { message, span: Span::new(at.min(self.n), self.n) }
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

        macro_rules! flush {
            () => {
                if !text.is_empty() {
                    nodes.push(Node::Text(std::mem::take(&mut text)));
                }
            };
        }

        while self.pos < self.n {
            let b = self.bytes[self.pos];
            if b == b'{' {
                flush!();
                match self.parse_tag()? {
                    TagResult::Node(node) => nodes.push(node),
                    TagResult::Term(term) => return Ok((nodes, term)),
                }
            } else if b == b'<' {
                if self.starts_with("<!--") {
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
                    text.push('<');
                    self.pos += 1;
                }
            } else {
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
            Ok(name)
        } else {
            Err(self.err_at(self.pos, format!("expected `>` to close `</{name}`")))
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
            return Err(self.err_at(start, format!("unclosed `<{tag}` tag")));
        };

        let kind = classify_element(&tag);
        let is_void = matches!(kind, ElementKind::Html) && VOID_ELEMENTS.contains(&tag.as_str());

        if self_close_syntax || is_void {
            return Ok(Element { tag, kind, attrs, children: Vec::new(), self_closing: true });
        }

        let (children, term) = self.parse_nodes()?;
        match term {
            Term::ElementClose(close) if close == tag => {
                Ok(Element { tag, kind, attrs, children, self_closing: false })
            }
            Term::ElementClose(other) => {
                Err(self.err_at(start, format!("`<{tag}>` closed by `</{other}>`")))
            }
            other => Err(self.err_at(start, format!("`<{tag}>` not closed (found {})", other.describe()))),
        }
    }

    fn parse_tag_name(&mut self) -> Result<String, ParseError> {
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
        Ok(self.src[start..self.pos].to_string())
    }

    fn parse_attrs(&mut self) -> Result<Vec<Attr>, ParseError> {
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            if self.pos >= self.n || self.bytes[self.pos] == b'>' || self.starts_with("/>") {
                break;
            }
            let name = self.parse_attr_name()?;
            self.skip_ws();
            if self.pos < self.n && self.bytes[self.pos] == b'=' {
                self.pos += 1;
                self.skip_ws();
                let value = match self.bytes.get(self.pos) {
                    Some(b'"') => AttrValue::Literal(self.parse_quoted(b'"')?),
                    Some(b'\'') => AttrValue::Literal(self.parse_quoted(b'\'')?),
                    Some(b'{') => AttrValue::Expr(self.parse_brace_inner()?),
                    _ => return Err(self.err_at(self.pos, format!("expected a value for attribute `{name}`"))),
                };
                attrs.push(Attr { name, value });
            } else {
                attrs.push(Attr { name, value: AttrValue::Boolean });
            }
        }
        Ok(attrs)
    }

    fn parse_attr_name(&mut self) -> Result<String, ParseError> {
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
        Ok(self.src[start..self.pos].to_string())
    }

    fn parse_quoted(&mut self, quote: u8) -> Result<String, ParseError> {
        let start = self.pos;
        self.pos += 1; // opening quote
        let content_start = self.pos;
        while self.pos < self.n && self.bytes[self.pos] != quote {
            self.pos += 1;
        }
        if self.pos >= self.n {
            return Err(self.err_at(start, "unterminated attribute string".into()));
        }
        let s = self.src[content_start..self.pos].to_string();
        self.pos += 1; // closing quote
        Ok(s)
    }

    /// Parse a `{ … }` group and return the trimmed inner text. `pos` must be at `{`.
    fn parse_brace_inner(&mut self) -> Result<String, ParseError> {
        let (inner, end) = self.scan_braces(self.pos)?;
        self.pos = end;
        Ok(inner.trim().to_string())
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
        Err(ParseError { message: "unclosed tag: missing `}`".into(), span: Span::new(open, self.n) })
    }

    fn parse_tag(&mut self) -> Result<TagResult, ParseError> {
        let open = self.pos;
        let (inner_raw, end) = self.scan_braces(open)?;
        self.pos = end;
        let t = inner_raw.trim();

        if let Some(body) = t.strip_prefix('@') {
            let body = body.trim_start();
            if let Some(rest) = keyword(body, "html") {
                return Ok(TagResult::Node(Node::Html(rest)));
            }
            if let Some(rest) = keyword(body, "render") {
                return Ok(TagResult::Node(Node::Render(rest)));
            }
            return Err(self.err_at(open, format!("unknown directive `{{@{body}}}`")));
        }

        if let Some(body) = t.strip_prefix('#') {
            let body = body.trim_start();
            if let Some(rest) = keyword(body, "use") {
                return Ok(TagResult::Node(Node::Use(rest)));
            }
            if let Some(cond) = keyword(body, "if") {
                return Ok(TagResult::Node(Node::If(self.parse_if(open, cond)?)));
            }
            if let Some(rest) = keyword(body, "each") {
                return Ok(TagResult::Node(Node::Each(self.parse_each(open, &rest)?)));
            }
            if let Some(rest) = keyword(body, "snippet") {
                return Ok(TagResult::Node(Node::Snippet(self.parse_snippet(open, &rest)?)));
            }
            return Err(self.err_at(open, format!("unknown block `{{#{body}}}`")));
        }

        if let Some(body) = t.strip_prefix(':') {
            let body = body.trim_start();
            if let Some(rest) = keyword(body, "else") {
                let rest = rest.trim_start();
                if let Some(cond) = keyword(rest, "if") {
                    return Ok(TagResult::Term(Term::ElseIf(cond)));
                }
                if rest.is_empty() {
                    return Ok(TagResult::Term(Term::Else));
                }
            }
            return Err(self.err_at(open, format!("unknown clause `{{:{body}}}`")));
        }

        if let Some(body) = t.strip_prefix('/') {
            return Ok(TagResult::Term(Term::TagClose(body.trim().to_string())));
        }

        if t.is_empty() {
            return Err(self.err_at(open, "empty tag `{}`".into()));
        }
        // A `{ … }` block — codegen decides value-vs-statement.
        Ok(TagResult::Node(Node::Expr(t.to_string())))
    }

    fn parse_if(&mut self, open: usize, first_cond: String) -> Result<IfNode, ParseError> {
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
                        Term::TagClose(k) if k == "if" => Ok(IfNode { branches, otherwise: Some(else_body) }),
                        other => Err(self.err_at(open, format!("`{{#if}}` not closed (found {})", other.describe()))),
                    };
                }
                Term::TagClose(k) if k == "if" => {
                    branches.push((cond, body));
                    return Ok(IfNode { branches, otherwise: None });
                }
                other => return Err(self.err_at(open, format!("`{{#if}}` not closed (found {})", other.describe()))),
            }
        }
    }

    fn parse_each(&mut self, open: usize, code: &str) -> Result<EachNode, ParseError> {
        let (expr, binding) = code
            .split_once(" as ")
            .ok_or_else(|| self.err_at(open, "`{#each … as …}` requires `as`".into()))?;
        let expr = expr.trim().to_string();
        let binding = binding.trim().to_string();
        if expr.is_empty() || binding.is_empty() {
            return Err(self.err_at(open, format!("malformed `{{#each {code}}}`")));
        }
        let (body, term) = self.parse_nodes()?;
        match term {
            Term::TagClose(k) if k == "each" => Ok(EachNode { expr, binding, body }),
            other => Err(self.err_at(open, format!("`{{#each}}` not closed (found {})", other.describe()))),
        }
    }

    fn parse_snippet(&mut self, open: usize, code: &str) -> Result<SnippetNode, ParseError> {
        let paren = code
            .find('(')
            .ok_or_else(|| self.err_at(open, "`{#snippet name(params)}` needs `(`".into()))?;
        let close = code
            .rfind(')')
            .ok_or_else(|| self.err_at(open, "`{#snippet name(params)}` needs `)`".into()))?;
        let name = code[..paren].trim().to_string();
        let params = code[paren + 1..close].trim().to_string();
        if name.is_empty() {
            return Err(self.err_at(open, "`{#snippet}` needs a name".into()));
        }
        let (body, term) = self.parse_nodes()?;
        match term {
            Term::TagClose(k) if k == "snippet" => Ok(SnippetNode { name, params, body }),
            other => Err(self.err_at(open, format!("`{{#snippet}}` not closed (found {})", other.describe()))),
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

fn keyword(s: &str, k: &str) -> Option<String> {
    let rest = s.strip_prefix(k)?;
    match rest.chars().next() {
        None => Some(String::new()),
        Some(c) if c.is_alphanumeric() || c == '_' => None,
        Some(_) => Some(rest.trim().to_string()),
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

/// Whether `offset` lies inside an open `{ … }` tag (brace depth > 0), tolerant
/// of half-typed tags. Used by the language server for completion context.
pub fn in_tag(src: &str, offset: usize) -> bool {
    let offset = offset.min(src.len());
    let bytes = src.as_bytes();
    let mut i = 0usize;
    let mut depth: i32 = 0;
    while i < offset {
        if bytes[i] == b'<' && src[i..].starts_with("<!--") {
            let end = src[i + 4..].find("-->").map(|r| i + 4 + r + 3).unwrap_or(src.len());
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
            vec![Node::Text("Hi ".into()), Node::Expr("self.name".into()), Node::Text("!".into())]
        );
        assert_eq!(nodes("{2 + 3; 10}"), vec![Node::Expr("2 + 3; 10".into())]);
        assert_eq!(nodes("{let a = 1}"), vec![Node::Expr("let a = 1".into())]);
    }

    #[test]
    fn directives_and_use() {
        assert_eq!(nodes("{@html x}"), vec![Node::Html("x".into())]);
        assert_eq!(nodes("{@render foo}"), vec![Node::Render("foo".into())]);
        assert_eq!(nodes("{#use crate::Card}"), vec![Node::Use("crate::Card".into())]);
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
                assert_eq!(el.attrs[0], Attr { name: "href".into(), value: AttrValue::Expr("self.url".into()) });
                assert_eq!(el.attrs[1], Attr { name: "class".into(), value: AttrValue::Literal("link".into()) });
                assert_eq!(el.attrs[2], Attr { name: "download".into(), value: AttrValue::Boolean });
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
                assert_eq!(el.attrs[0], Attr { name: "title".into(), value: AttrValue::Expr("2 + 8".into()) });
                // children: text "default", then a <slot name="foot"> element
                assert_eq!(el.children.len(), 2);
                match &el.children[1] {
                    Node::Element(slot) => {
                        assert_eq!(slot.kind, ElementKind::Slot);
                        assert_eq!(slot.attrs[0], Attr { name: "name".into(), value: AttrValue::Literal("foot".into()) });
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
                assert_eq!(el.attrs[0].value, AttrValue::Expr(r#"Bar { x: "}".into() }"#.into()));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn mismatched_close_errors() {
        assert!(parse("<div>{#if x}</div>{/if}").is_err());
        assert!(parse("<div>hi</span>").unwrap_err().message.contains("closed by"));
    }

    #[test]
    fn html_comment_is_text() {
        assert_eq!(nodes("a<!-- {x} -->b"), vec![Node::Text("a<!-- {x} -->b".into())]);
    }

    #[test]
    fn in_tag_detection() {
        assert!(in_tag("Hi { self.na", 12));
        assert!(!in_tag("Hi { x } bye", 12));
        assert!(in_tag("<Foo a={Bar { ", 14));
    }
}
