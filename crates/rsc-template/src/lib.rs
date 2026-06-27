//! Template parser for **RSC** (Rust Smart Components).
//!
//! This crate turns a `.rsc` template into a flat list of [`Node`]s. It is the
//! single source of truth for template syntax: both the `component!` macro
//! (code generation) and the language server (highlighting / diagnostics /
//! completion) depend on it, so the two can never disagree about what a
//! template means.
//!
//! The parser is intentionally small and dependency-free. It does **not** parse
//! the Rust inside a tag or the host language outside one — it only splits a
//! template into literal text and tags, recording byte spans so callers can map
//! results back to source positions.
//!
//! # Tags
//!
//! | Syntax        | [`TagKind`]        | Meaning                                        |
//! |---------------|--------------------|------------------------------------------------|
//! | `<%= expr %>` | [`TagKind::Escaped`]   | write `expr` (escaped per the renderer)    |
//! | `<%- expr %>` | [`TagKind::Raw`]       | write `expr` (unescaped `Display`)         |
//! | `<%+ expr %>` | [`TagKind::Render`]    | render a child component into the output   |
//! | `<% stmt %>`  | [`TagKind::Statement`] | splice Rust statement(s)                   |
//! | `<%# text %>` | [`TagKind::Comment`]   | dropped entirely                           |
//!
//! `<%%` and `%%>` are literal escapes for `<%` and `%>` in text.

mod host;
mod line_index;
mod parser;

pub use host::HostLang;
pub use line_index::LineIndex;
pub use parser::{ParseError, ParseOptions, parse};

/// A half-open byte range `[start, end)` into the template source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start, end }
    }

    /// The slice of `src` this span refers to.
    ///
    /// Spans are always produced from the `src` that was parsed, so this cannot
    /// panic for a span obtained from [`parse`] over the same string.
    pub fn slice<'a>(&self, src: &'a str) -> &'a str {
        &src[self.start..self.end]
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// The kind of a `<% … %>` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    /// `<%= expr %>` — write `expr` applying the renderer's escaping policy.
    Escaped,
    /// `<%- expr %>` — write `expr` verbatim (raw `Display`).
    Raw,
    /// `<%+ expr %>` — render a child [`Component`] into the current output.
    ///
    /// [`Component`]: https://docs.rs/rsc
    Render,
    /// `<% stmt %>` — splice Rust statement(s) into the generated body.
    Statement,
    /// `<%# text %>` — a comment; emits nothing.
    Comment,
}

impl TagKind {
    /// `true` for tags whose body is a Rust *expression* (`<%=`, `<%-`, `<%+`).
    pub fn is_expression(self) -> bool {
        matches!(self, TagKind::Escaped | TagKind::Raw | TagKind::Render)
    }
}

/// A single piece of a parsed template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// Literal host-language text (with `<%%`/`%%>` escapes already resolved).
    Text {
        /// Span of the original source (before escape resolution).
        span: Span,
        /// The literal text to emit.
        text: String,
    },
    /// A `<% … %>` tag.
    Tag {
        /// Span of the whole tag, delimiters included.
        span: Span,
        kind: TagKind,
        /// The (whitespace-trimmed) Rust/comment source between the delimiters.
        code: String,
        /// Span of `code` within the source (delimiters excluded).
        code_span: Span,
    },
}

/// A fully parsed template: an ordered list of text and tag nodes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Template {
    pub nodes: Vec<Node>,
}

impl Template {
    /// Iterate over just the tag nodes (useful for the LSP / diagnostics).
    pub fn tags(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| matches!(n, Node::Tag { .. }))
    }
}

/// Convert a `PascalCase` component name to the `snake_case` used for its
/// template's file-name convention (`HTMLPage` → `html_page`).
///
/// Shared by the `component` derive (name → template file) and the language
/// server (template file → struct name), so the two agree on pairing.
pub fn to_snake_case(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                let next_is_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
                if prev.is_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_uppercase() && next_is_lower)
                {
                    out.push('_');
                }
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod snake_tests {
    use super::to_snake_case;

    #[test]
    fn common_shapes() {
        assert_eq!(to_snake_case("Greeting"), "greeting");
        assert_eq!(to_snake_case("MyButton"), "my_button");
        assert_eq!(to_snake_case("HTMLPage"), "html_page");
        assert_eq!(to_snake_case("Card2Col"), "card2_col");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }
}
