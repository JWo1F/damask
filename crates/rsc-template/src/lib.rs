//! Template parser for **RSC** (Rust Smart Components).
//!
//! RSC templates are HTML with a `{ … }` tag syntax. This crate turns a
//! `.rsc` template into a flat list of [`Node`]s — literal text and classified
//! tags — recording byte spans so callers can map results back to source. It is
//! the single source of truth for template syntax, shared by the `Component`
//! derive (code generation) and the language server.
//!
//! It does **not** parse the Rust inside a tag; it only splits the template into
//! text and tags and classifies each tag.
//!
//! # Tags
//!
//! | Syntax                     | [`TagKind`]           | Meaning                        |
//! |----------------------------|-----------------------|--------------------------------|
//! | `{ expr }`                 | [`TagKind::Expr`]     | write `expr`, HTML-escaped     |
//! | `{@html expr}`             | [`TagKind::Html`]     | write `expr` raw               |
//! | `{@const x = e}`           | [`TagKind::Const`]    | a local `let` binding          |
//! | `{@render expr}`           | [`TagKind::Render`]   | render a child / snippet       |
//! | `{#if c}`                  | [`TagKind::If`]       | conditional open               |
//! | `{:else if c}` / `{:else}` | [`TagKind::ElseIf`] / [`TagKind::Else`] | conditional clause |
//! | `{#each E as p[, i]}`       | [`TagKind::Each`]     | loop open                      |
//! | `{#snippet name(params)}`  | [`TagKind::Snippet`]  | define a reusable fragment     |
//! | `{/if}` `{/each}` `{/snippet}` | [`TagKind::Close`] | block close                    |
//!
//! Literal braces are written as expressions (`{"{"}`); `<!-- … -->` comments
//! pass through as text.

mod line_index;
mod parser;

pub use line_index::LineIndex;
pub use parser::{ParseError, in_tag, parse};

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

/// The kind of a `{ … }` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    /// `{ expr }` — write `expr`, HTML-escaped.
    Expr,
    /// `{@html expr}` — write `expr` verbatim.
    Html,
    /// `{@const name = expr}` — a local `let` binding for the enclosing block.
    Const,
    /// `{@render expr}` — render a child component or snippet into the output.
    Render,
    /// `{#if cond}` — open a conditional.
    If,
    /// `{:else if cond}` — conditional continuation.
    ElseIf,
    /// `{:else}` — conditional fallthrough.
    Else,
    /// `{#each expr as pat[, index]}` — open a loop.
    Each,
    /// `{#snippet name(params)}` — define a reusable fragment.
    Snippet,
    /// `{/if}`, `{/each}`, `{/snippet}` — close a block. `code` holds the keyword.
    Close,
}

impl TagKind {
    /// Whether this tag opens a block that must be closed (`{#…}`).
    pub fn opens_block(self) -> bool {
        matches!(self, TagKind::If | TagKind::Each | TagKind::Snippet)
    }
}

/// A single piece of a parsed template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// Literal HTML text.
    Text { span: Span, text: String },
    /// A `{ … }` tag.
    Tag {
        /// Span of the whole tag, braces included.
        span: Span,
        kind: TagKind,
        /// The meaningful inner content — sigil and block keyword stripped.
        /// For [`TagKind::Close`] it is the keyword (`if` / `each` / `snippet`).
        code: String,
        /// Span of `code` within the source.
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
/// template's file name (`HTMLPage` → `html_page`).
///
/// Shared by the `Component` derive (name → template file) and the language
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
