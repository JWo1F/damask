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
//! | `{ expr }` | Rust block — prints its value (escaped), or nothing if it's a statement (e.g. `{use crate::X}`, `{let x = e}`) |
//! | `{@html expr}` | write `expr` raw |
//! | `{@render expr}` | render a snippet / fragment |
//! | `{#if c}…{:else if c}…{:else}…{/if}` | conditional |
//! | `{#each E as p[, i]}…{/each}` | loop |
//! | `{#snippet name(params)}…{/snippet}` | define a reusable fragment |
//!
//! # Elements
//!
//! - `<div>…</div>` — an HTML element (lowercase). Its content is a scope.
//! - `<Component attr={expr}>…</Component>` — a component (capitalized): built
//!   and rendered; attributes become fields, content fills slots.
//! - `<slot/>` / `<slot name="x">fallback</slot>` — a slot: renders what the
//!   caller passed for that name, or the `<slot>`'s own body if unfilled. A
//!   *named* `<slot>` directly inside a `<Component>` instead fills that name;
//!   a bare `<slot/>` there is still a placeholder, so it forwards this
//!   component's default slot into the child's.

mod line_index;
mod lower;
mod parser;

pub use line_index::LineIndex;
pub use lower::{Mapping, SourceMap, lower, lower_mapped};
pub use parser::{ParseError, in_tag, is_void_element, parse, tag_spans};

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

/// A string extracted from the template — a Rust code fragment or a run of
/// literal text — paired with the [`Span`] it came from. The span is what lets
/// the language server map a position in a generated virtual file back to the
/// exact byte range in the `.rsc` source.
///
/// Structural equality (and `Hash`) compares only `text`, so `Node` trees can be
/// compared by shape without threading positions through every test; span
/// correctness is checked with dedicated tests that read `.span` directly.
#[derive(Debug, Clone)]
pub struct Spanned {
    pub text: String,
    pub span: Span,
}

impl Spanned {
    pub fn new(text: impl Into<String>, span: Span) -> Self {
        Spanned {
            text: text.into(),
            span,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl PartialEq for Spanned {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Spanned {}

impl std::hash::Hash for Spanned {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl std::fmt::Display for Spanned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// A zero-span `Spanned`; convenient for construction in tests and for synthetic
/// fragments that don't correspond to any source range.
impl From<&str> for Spanned {
    fn from(text: &str) -> Self {
        Spanned::new(text, Span::new(0, 0))
    }
}

impl From<String> for Spanned {
    fn from(text: String) -> Self {
        Spanned::new(text, Span::new(0, 0))
    }
}

impl PartialEq<&str> for Spanned {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<str> for Spanned {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

/// A node in the template tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// Literal HTML text.
    Text(Spanned),
    /// A `{ … }` tag holding a Rust block body. Codegen prints its value if it
    /// is an expression, or splices it (no output) if it is a statement /
    /// binding — `{ self.name }`, `{ 2 + 3; 10 }`, `{ let x = e }`.
    Expr(Spanned),
    /// `{@html expr}` — raw output.
    Html(Spanned),
    /// `{@render expr}` — render a snippet / fragment.
    Render(Spanned),
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
    pub branches: Vec<(Spanned, Vec<Node>)>,
    /// The `{:else}` body, if any.
    pub otherwise: Option<Vec<Node>>,
}

/// `{#each expr as binding}…{/each}` (binding may be `pat` or `pat, index`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EachNode {
    pub expr: Spanned,
    pub binding: Spanned,
    pub body: Vec<Node>,
}

/// `{#snippet name(params)}…{/snippet}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetNode {
    pub name: Spanned,
    pub params: Spanned,
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
    /// Spanned so the language server can map a component name back to the
    /// struct it lowers to (see [`lower`]); HTML tag names are never mapped.
    pub tag: Spanned,
    pub kind: ElementKind,
    pub attrs: Vec<Attr>,
    pub children: Vec<Node>,
    /// `true` for `<x/>` or a void HTML element (no children).
    pub self_closing: bool,
}

/// An element attribute, or a `{...expr}` spread of several.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    /// Spanned for the same reason as [`Element::tag`]: on a component this
    /// name lowers to a struct field, so the server can resolve it.
    pub name: Spanned,
    pub value: AttrValue,
}

/// The value of an attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    /// `name="text"` or `name='text'`, which may interpolate: the quoted run is
    /// a list of parts rather than one string, so `class="px-3 {self.tone()}"`
    /// is a literal and an expression side by side.
    ///
    /// A quoted value is always emitted — the quotes say the attribute is
    /// there, whatever its parts evaluate to. Omission is [`AttrValue::Expr`]'s
    /// job, where the type of the expression can ask for it.
    Literal(Vec<AttrPart>),
    /// `name={expr}`.
    Expr(Spanned),
    /// A bare `name` (boolean).
    Boolean,
    /// `{...expr}` — a run of attributes prepared elsewhere.
    ///
    /// The escape hatch for the attributes a component cannot name: those whose
    /// *name* is computed (`data-<controller>-target`) or that arrive as a map.
    /// The expression yields anything implementing `rsc::AttrSpread`, which is
    /// responsible for its own escaping — which is why the trait is implemented
    /// for a key/value map and for `&'static str`, but not for `String`.
    Spread(Spanned),
    /// A class list: `class=[…]` or `class={ "a": cond }`.
    ///
    /// Only `class` parses this way. The forms are not Rust — `{ "a": cond }`
    /// is neither a block nor a struct literal — so they are a grammar of their
    /// own rather than an expression handed to the compiler, and giving every
    /// attribute that grammar would make `foo={ … }` ambiguous for no gain.
    Classes(Vec<ClassTerm>),
}

/// One entry in a `class` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassTerm {
    /// A Rust expression yielding something string-ish, or an `Option` of one.
    Expr(Spanned),
    /// A literal `None`, which contributes nothing.
    ///
    /// Recognised as a token rather than evaluated: a bare `None` has no type
    /// to infer from, so `[Some("a"), None]` would not compile if it were
    /// lowered as an expression. Dropping it here is also what it means.
    Nothing,
    /// `"name": cond` — the class is present while `cond` holds.
    Cond { name: Spanned, when: Spanned },
}

impl AttrValue {
    /// A quoted value with no interpolation — the common case, and the one
    /// worth a constructor because it is otherwise a one-element `Vec`.
    pub fn text(s: impl Into<String>) -> Self {
        AttrValue::Literal(vec![AttrPart::Text(Spanned::new(s.into(), Span::new(0, 0)))])
    }

    /// The value as one static string, when it is one — `None` for anything
    /// that has to be evaluated.
    pub fn as_static_text(&self) -> Option<&str> {
        match self {
            AttrValue::Literal(parts) => match parts.as_slice() {
                [AttrPart::Text(t)] => Some(t.as_str()),
                _ => None,
            },
            _ => None,
        }
    }
}

/// One piece of a quoted attribute value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrPart {
    /// Literal text between the interpolations.
    Text(Spanned),
    /// A `{ … }` hole, printed escaped.
    Expr(Spanned),
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
