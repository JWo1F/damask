//! Template parser for **RSC** (Rust Smart Components).
//!
//! RSC templates are HTML with a `{ … }` tag syntax, JSX-style
//! `<Component/>` elements, and `<slot/>`s. This crate parses a `.rsc` template
//! into a [`Node`] tree — it is the single source of truth for template syntax,
//! shared by the `Component` derive (code generation) and the language server.
//!
//! It does **not** parse the Rust inside a tag or attribute; it extracts the
//! Rust as text and leaves type-checking to `rustc`.
//!
//! # Tags
//!
//! | Syntax | Meaning |
//! |--------|---------|
//! | `{ expr }` | Rust block — prints its value (escaped), or nothing if it's a statement |
//! | `{@html expr}` | write `expr` raw |
//! | `{@render expr}` | render a snippet / fragment |
//! | `{#use path}` | a Rust `use`, scoped to the enclosing element |
//! | `{#if c}…{:else if c}…{:else}…{/if}` | conditional |
//! | `{#each E as p[, i]}…{/each}` | loop |
//! | `{#snippet name(params)}…{/snippet}` | define a reusable fragment |
//!
//! # Elements
//!
//! - `<div>…</div>` — an HTML element (lowercase). Its content is a scope.
//! - `<Component attr={expr}>…</Component>` — a component (capitalized): built
//!   and rendered; attributes become fields, children fill slots.
//! - `<slot/>` / `<slot name="x"/>` — a slot: in a component it renders the
//!   passed children; as a child of a `<Component>` it fills a named slot.

mod line_index;
mod parser;

pub use line_index::LineIndex;
pub use parser::{ParseError, in_tag, is_void_element, parse};

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

/// A node in the template tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// Literal HTML text.
    Text(String),
    /// A `{ … }` tag holding a Rust block body. Codegen prints its value if it
    /// is an expression, or splices it (no output) if it is a statement /
    /// binding — `{ self.name }`, `{ 2 + 3; 10 }`, `{ let x = e }`.
    Expr(String),
    /// `{@html expr}` — raw output.
    Html(String),
    /// `{@render expr}` — render a snippet / fragment.
    Render(String),
    /// `{#use path}` — a Rust `use`, scoped to the enclosing element.
    Use(String),
    /// `{#if …}…{/if}`.
    If(IfNode),
    /// `{#each …}…{/each}`.
    Each(EachNode),
    /// `{#snippet …}…{/snippet}`.
    Snippet(SnippetNode),
    /// An HTML element, component, or slot.
    Element(Element),
}

/// `{#if}` with its branches and optional trailing `{:else}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IfNode {
    /// One entry per `{#if}` / `{:else if}` condition, with its body.
    pub branches: Vec<(String, Vec<Node>)>,
    /// The `{:else}` body, if any.
    pub otherwise: Option<Vec<Node>>,
}

/// `{#each expr as binding}…{/each}` (binding may be `pat` or `pat, index`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EachNode {
    pub expr: String,
    pub binding: String,
    pub body: Vec<Node>,
}

/// `{#snippet name(params)}…{/snippet}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetNode {
    pub name: String,
    pub params: String,
    pub body: Vec<Node>,
}

/// The three kinds of `<…>` element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    /// Lowercase tag, e.g. `<div>` — emitted as HTML; its content is a scope.
    Html,
    /// Capitalized tag, e.g. `<Card>` — a component to build and render.
    Component,
    /// `<slot>` — a slot placeholder / fill.
    Slot,
}

/// A parsed element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub tag: String,
    pub kind: ElementKind,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
    /// `true` for `<x/>` or a void HTML element (no children).
    pub self_closing: bool,
}

/// An element attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: String,
    pub value: AttrValue,
}

/// The value of an attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    /// `name="text"` or `name='text'`.
    Literal(String),
    /// `name={expr}`.
    Expr(String),
    /// A bare `name` (boolean).
    Boolean,
}

/// A parsed template: the top-level node list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Template {
    pub nodes: Vec<Node>,
}

/// Convert a `PascalCase` component name to the `snake_case` used for its
/// template's file name (`HTMLPage` → `html_page`).
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
    }
}
