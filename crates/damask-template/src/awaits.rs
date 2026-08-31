//! Whether a template needs an async render path.
//!
//! A template is async exactly when one of its own Rust fragments contains
//! `.await` — text tags, `{#if}`/`{#for}` conditions and iterators,
//! attribute/class/data expressions, and snippet bodies (recursively) — or
//! when a component element is marked `await`.
//!
//! That marker exists because detection cannot look inside a nested component:
//! a `<Card/>` is a type, and whether it renders asynchronously is something
//! rustc knows and this scan cannot, on the other side of a macro-expansion
//! boundary. Without a way to say so, an async-only child was unusable unless
//! the enclosing template happened to await something else — so `<Card await/>`
//! is the author saying what only they can know.
//!
//! Detection walks the parsed [`Template`] rather than the generated Rust
//! string, and tokenizes each fragment with `proc_macro2` rather than
//! searching its text, so a string literal that happens to contain the bytes
//! `.await` (`{ "user.await" }`) is not mistaken for the genuine token.

use crate::{Attr, AttrPart, AttrValue, ClassTerm, DataTerm, Node, Spanned, Template};
use proc_macro2::{TokenStream, TokenTree};

/// Whether `template` contains `.await` anywhere in its own Rust fragments.
pub fn needs_async(template: &Template) -> bool {
    nodes_need_async(&template.nodes)
}

pub(crate) fn nodes_need_async(nodes: &[Node]) -> bool {
    nodes.iter().any(node_needs_async)
}

pub(crate) fn node_needs_async(node: &Node) -> bool {
    match node {
        Node::Text(_) => false,
        Node::Expr(code) | Node::Html(code) | Node::Render(code) => fragment_awaits(code),
        Node::If(if_node) => {
            if_node
                .branches
                .iter()
                .any(|(cond, body)| fragment_awaits(cond) || nodes_need_async(body))
                || if_node.otherwise.as_deref().is_some_and(nodes_need_async)
        }
        Node::For(for_node) => {
            fragment_awaits(&for_node.pat)
                || fragment_awaits(&for_node.expr)
                || nodes_need_async(&for_node.body)
        }
        Node::Snippet(snippet) => nodes_need_async(&snippet.body),
        Node::Element(el) => {
            is_awaited(el)
                || el.attrs.iter().any(attr_needs_async)
                || nodes_need_async(&el.children)
        }
    }
}

/// Whether this element carries the bare `await` marker.
pub(crate) fn is_awaited(el: &crate::Element) -> bool {
    el.attrs
        .iter()
        .any(|attr| attr.name.as_str() == "await" && matches!(attr.value, AttrValue::Boolean))
}

fn attr_needs_async(attr: &Attr) -> bool {
    match &attr.value {
        AttrValue::Literal(parts) => parts.iter().any(|p| match p {
            AttrPart::Text(_) => false,
            AttrPart::Expr(code) => fragment_awaits(code),
        }),
        AttrValue::Expr(code) | AttrValue::Spread(code) => fragment_awaits(code),
        AttrValue::Boolean => false,
        AttrValue::Classes(terms) => terms.iter().any(|t| match t {
            ClassTerm::Expr(code) => fragment_awaits(code),
            ClassTerm::Nothing => false,
            ClassTerm::Cond { when, .. } => fragment_awaits(when),
        }),
        AttrValue::Data(terms) => terms.iter().any(|t| match t {
            DataTerm::Expr(code) => fragment_awaits(code),
            DataTerm::Nothing => false,
            DataTerm::Pair { value, .. } => fragment_awaits(value),
        }),
    }
}

/// Whether one Rust fragment contains `.await`.
///
/// A fragment that fails to tokenize is not this pass's problem to report —
/// lowering will hit the same text and produce the real error — so it is
/// treated as await-free here.
fn fragment_awaits(code: &Spanned) -> bool {
    match code.as_str().parse::<TokenStream>() {
        Ok(ts) => stream_awaits(ts),
        Err(_) => false,
    }
}

fn stream_awaits(ts: TokenStream) -> bool {
    let mut prev_dot = false;
    for tt in ts {
        match tt {
            TokenTree::Punct(p) => {
                prev_dot = p.as_char() == '.';
            }
            TokenTree::Ident(id) => {
                if prev_dot && id == "await" {
                    return true;
                }
                prev_dot = false;
            }
            TokenTree::Group(g) => {
                prev_dot = false;
                if stream_awaits(g.stream()) {
                    return true;
                }
            }
            TokenTree::Literal(_) => {
                prev_dot = false;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(src: &str) -> Template {
        crate::parse(src).unwrap()
    }

    #[test]
    fn plain_template_is_sync() {
        assert!(!needs_async(&parsed("Hello {self.name}!")));
    }

    #[test]
    fn direct_await_in_expr_tag_is_async() {
        assert!(needs_async(&parsed("{self.fetch().await}")));
    }

    #[test]
    fn await_inside_string_literal_is_not_async() {
        assert!(!needs_async(&parsed(r#"{ "user.await" }"#)));
    }

    #[test]
    fn await_in_if_condition_is_async() {
        assert!(needs_async(&parsed("{#if self.check().await}yes{/if}")));
    }

    #[test]
    fn await_in_for_iterator_is_async() {
        assert!(needs_async(&parsed(
            "{#for item in self.items().await}{item}{/for}"
        )));
    }

    #[test]
    fn await_in_attribute_expr_is_async() {
        assert!(needs_async(&parsed(
            r#"<div title={self.title().await}></div>"#
        )));
    }

    #[test]
    fn await_in_snippet_body_is_async() {
        assert!(needs_async(&parsed(
            "{#snippet item()}{self.fetch().await}{/snippet}{@render item()}"
        )));
    }

    #[test]
    fn await_in_nested_element_is_async() {
        assert!(needs_async(&parsed(
            "<div><span>{self.fetch().await}</span></div>"
        )));
    }
}
