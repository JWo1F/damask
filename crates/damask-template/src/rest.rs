//! Which attributes on a component are props, and which are attributes it does
//! not name.
//!
//! This pass cannot answer that question, and does not try. A `<Hidden …/>` is
//! lowered here and `Hidden` is compiled somewhere else, so the set of props it
//! declares is not visible from this side of the macro-expansion boundary — the
//! same wall that makes `<Card await/>` a marker the author has to write.
//!
//! So the decision is deferred to method resolution, which sees both. For every
//! attribute whose name *could* be a prop, lowering emits the setter call it
//! always emitted, and next to it a one-method trait of the same name whose
//! impl routes to the component's bag. Rust prefers an inherent method to a
//! trait one, so a declared prop takes its own setter and only a name the
//! builder has no setter for reaches the trait. A component with no
//! `#[prop(rest)]` field does not implement [`Rest`](damask::props::Rest) at
//! all, and the bound is what refuses the attribute — which is how a typo is
//! still a build failure.
//!
//! That bound is on the **impl**, `impl<T: Rest>` rather than a blanket impl
//! with a `where Self: Rest` on each method, and the difference is what the
//! rest of the template sees: a setter takes `self` by value, so the fallback
//! does too, and a by-value candidate is picked ahead of an autoref one — so an
//! unbounded impl was chosen over an inherent `&self` method of the same name
//! anywhere in the same template. Bounding the impl leaves the fallback
//! inapplicable to anything that is not a props builder, which is exactly the
//! set it was written for.
//!
//! The names this leaves behind are the ones that could not be a method in the
//! first place: everything hyphenated (`data-cover-target`, `aria-label`), and
//! every Rust keyword (`type`, `for`, `async`). Those are emitted as calls to
//! the bag directly, and are why `<TextInput type="email"/>` works even though
//! no field can be called `type`.

use crate::{AttrValue, ElementKind, Node};

/// Rust's keywords, reserved words included. A name in here cannot be a method,
/// so it goes to the bag — which is the useful reading anyway, since `type`,
/// `for` and `async` are all real HTML attributes and none of them could ever
/// have been a prop.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield", "Self",
];

/// Whether `name` could name a prop — that is, whether it could be a method.
///
/// Ident-shaped and not a keyword. Everything else can only be an attribute,
/// and is emitted straight to the bag.
pub(crate) fn could_be_a_prop(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !KEYWORDS.contains(&name)
}

/// Whether any component in `nodes` was given an attribute that can only go to
/// the bag — a hyphenated name, a keyword, or a `{...}` spread. Those reach the
/// bag by name rather than by resolution, so they need the blanket trait in
/// [`any_trait`] rather than a fallback per name.
pub(crate) fn needs_any_trait(nodes: &[Node]) -> bool {
    fn walk(nodes: &[Node]) -> bool {
        nodes.iter().any(|node| match node {
            Node::Text(_) | Node::Expr(_) | Node::Html(_) | Node::Render(_) => false,
            Node::If(node) => {
                node.branches.iter().any(|(_, body)| walk(body))
                    || node.otherwise.as_deref().is_some_and(walk)
            }
            Node::For(node) => walk(&node.body),
            Node::Snippet(node) => walk(&node.body),
            Node::Element(el) => {
                (el.kind == ElementKind::Component
                    && el.attrs.iter().any(|attr| match &attr.value {
                        AttrValue::Spread(_) => true,
                        // The `await` marker is not an attribute at all, and is
                        // dropped before any of this.
                        AttrValue::Boolean if attr.name.as_str() == "await" => false,
                        _ => !could_be_a_prop(attr.name.as_str()),
                    }))
                    || walk(&el.children)
            }
        })
    }
    walk(nodes)
}

/// Every distinct attribute name on a component in `nodes` that could be a
/// prop — one fallback trait is emitted per entry, in the order first seen so
/// that the generated code is stable.
pub(crate) fn fallback_names(nodes: &[Node]) -> Vec<String> {
    let mut names = Vec::new();
    collect(nodes, &mut names);
    names
}

fn collect(nodes: &[Node], names: &mut Vec<String>) {
    for node in nodes {
        match node {
            Node::Text(_) | Node::Expr(_) | Node::Html(_) | Node::Render(_) => {}
            Node::If(node) => {
                for (_, body) in &node.branches {
                    collect(body, names);
                }
                if let Some(body) = &node.otherwise {
                    collect(body, names);
                }
            }
            Node::For(node) => collect(&node.body, names),
            Node::Snippet(node) => collect(&node.body, names),
            Node::Element(el) => {
                if el.kind == ElementKind::Component {
                    for attr in &el.attrs {
                        let name = attr.name.as_str();
                        // The `await` marker is not a prop and never reaches a
                        // setter, so a fallback for it would route a keyword
                        // that was already handled.
                        if name == "await" && matches!(attr.value, AttrValue::Boolean) {
                            continue;
                        }
                        if could_be_a_prop(name) && !names.iter().any(|seen| seen == name) {
                            names.push(name.to_string());
                        }
                    }
                }
                collect(&el.children, names);
            }
        }
    }
}

/// The fallback trait for one name, defined and implemented for every builder.
///
/// Emitted inside the render function's own block, so it is scoped to this one
/// template: two templates in the same module cannot collide over a name, and
/// nothing leaks into the crate that wrote them.
///
/// **The bound is on the impl and not on the methods**, which is what keeps the
/// fallback out of the way of the template's own code. A setter takes `self` by
/// value, so this one must too — and a by-value candidate is picked before an
/// autoref one, so a blanket impl would win against an inherent `&self` method
/// of the same name anywhere in the template: a `<Icon class="…"/>` somewhere
/// in the file made `{link.class()}` on a plain `<a>` resolve to a setter for a
/// bag, and reported it as three errors about a component nobody wrote. Bounding
/// the impl makes the candidate inapplicable to a type that is not a props
/// builder, so method resolution never considers it and the inherent method is
/// found. The diagnostic is unchanged: the message still comes from `Rest`'s
/// `on_unimplemented`, as an unsatisfied bound on the impl rather than on the
/// method.
pub(crate) fn fallback_trait(name: &str, krate: &str) -> String {
    format!(
        "#[allow(non_camel_case_types)]\n\
         trait __DamaskRest_{name} {{\n\
         fn {name}<__DamaskValue: {krate}::attr::IntoAttrValue>(self, value: __DamaskValue) -> Self;\n\
         fn __damask_literal_{name}(self, text: &'static str) -> Self;\n\
         fn __damask_text_{name}(self, text: ::std::string::String) -> Self;\n\
         }}\n\
         impl<__DamaskAny: {krate}::props::Rest> __DamaskRest_{name} for __DamaskAny {{\n\
         fn {name}<__DamaskValue: {krate}::attr::IntoAttrValue>(self, value: __DamaskValue) -> Self {{\n\
         {krate}::props::Rest::__damask_rest(self, {name:?}, value)\n\
         }}\n\
         fn __damask_literal_{name}(self, text: &'static str) -> Self {{\n\
         {krate}::props::Rest::__damask_rest_static(self, {name:?}, text)\n\
         }}\n\
         fn __damask_text_{name}(self, text: ::std::string::String) -> Self {{\n\
         {krate}::props::Rest::__damask_rest(self, {name:?}, text)\n\
         }}\n\
         }}\n"
    )
}

/// The blanket trait every attribute that can only go to the bag travels
/// through.
///
/// It exists for the diagnostic. A hyphenated name needs no fallback to be
/// chosen — no setter could ever be called `data-cover-target` — so lowering
/// could call [`Rest`](damask::props::Rest) directly. But then a component
/// *without* a bag fails with "method not found", naming a generated builder
/// nobody wrote. Routing through a blanket impl whose methods are bounded
/// `where Self: Rest` moves the failure onto the bound instead, which is the
/// one place the message can be phrased.
pub(crate) fn any_trait(krate: &str) -> String {
    format!(
        "#[allow(non_camel_case_types)]\n\
         trait __DamaskRestAny {{\n\
         fn __damask_rest_any<__DamaskValue: {krate}::attr::IntoAttrValue>(self, name: &'static str, value: __DamaskValue) -> Self where Self: {krate}::props::Rest;\n\
         fn __damask_rest_static_any(self, name: &'static str, value: &'static str) -> Self where Self: {krate}::props::Rest;\n\
         fn __damask_rest_bare_any(self, name: &'static str) -> Self where Self: {krate}::props::Rest;\n\
         fn __damask_rest_spread_any<__DamaskAttrs: {krate}::attr::AttrSet + ?Sized>(self, attrs: &__DamaskAttrs) -> Self where Self: {krate}::props::Rest;\n\
         }}\n\
         impl<__DamaskAny> __DamaskRestAny for __DamaskAny {{\n\
         fn __damask_rest_any<__DamaskValue: {krate}::attr::IntoAttrValue>(self, name: &'static str, value: __DamaskValue) -> Self where Self: {krate}::props::Rest {{\n\
         {krate}::props::Rest::__damask_rest(self, name, value)\n\
         }}\n\
         fn __damask_rest_static_any(self, name: &'static str, value: &'static str) -> Self where Self: {krate}::props::Rest {{\n\
         {krate}::props::Rest::__damask_rest_static(self, name, value)\n\
         }}\n\
         fn __damask_rest_bare_any(self, name: &'static str) -> Self where Self: {krate}::props::Rest {{\n\
         {krate}::props::Rest::__damask_rest_bare(self, name)\n\
         }}\n\
         fn __damask_rest_spread_any<__DamaskAttrs: {krate}::attr::AttrSet + ?Sized>(self, attrs: &__DamaskAttrs) -> Self where Self: {krate}::props::Rest {{\n\
         {krate}::props::Rest::__damask_rest_spread(self, attrs)\n\
         }}\n\
         }}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::could_be_a_prop;

    #[test]
    fn a_hyphenated_or_keyword_name_can_only_be_an_attribute() {
        for name in ["class", "of", "rows", "_private", "dataX"] {
            assert!(could_be_a_prop(name), "`{name}` could be a prop");
        }
        for name in [
            "data-cover-target",
            "aria-label",
            "x-on:click",
            "type",
            "for",
            "async",
            "",
        ] {
            assert!(!could_be_a_prop(name), "`{name}` could not be a prop");
        }
    }
}
