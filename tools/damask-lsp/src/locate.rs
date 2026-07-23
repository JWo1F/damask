//! Locate the Damask entity under the cursor by walking the parsed template.
//!
//! This is the component-facing counterpart to [`crate::analysis`]: where that
//! classifies a half-typed tag for completion, this reads a *parsed* template to
//! answer hover and go-to-definition over the surface rust-analyzer cannot
//! explain — a component's own name, its attributes (which lower to generated
//! builder setters), and its slots (which are matched by string at run time and
//! lower to nothing rust-analyzer sees).

use damask_template::{AttrValue, Element, ElementKind, Node, Span, Template};

/// What the cursor sits on, when it is a Damask-specific entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A `<Component>` tag name.
    ComponentName { name: String, span: Span },
    /// An attribute *name* on a component element.
    ComponentAttr {
        component: String,
        attr: String,
        span: Span,
    },
    /// The `name` value of a `<slot name="x">` declaration (`None` for a bare
    /// `<slot/>`, the default slot).
    SlotName { name: Option<String>, span: Span },
    /// A `slot="x"` fill value. `component` is the nearest ancestor component
    /// element — the one whose slot is being filled — when there is one.
    SlotFill {
        component: Option<String>,
        name: String,
        span: Span,
    },
}

/// Find the innermost [`Target`] at `offset`, or `None` for a position that is
/// plain markup, Rust code, or otherwise not a component/slot entity.
pub fn locate(template: &Template, offset: usize) -> Option<Target> {
    locate_nodes(&template.nodes, offset, None)
}

/// `within` names the nearest ancestor component element, so a `slot="…"` fill
/// on a child resolves to the component whose slot it fills.
fn locate_nodes(nodes: &[Node], offset: usize, within: Option<&str>) -> Option<Target> {
    for node in nodes {
        let hit = match node {
            Node::Element(el) => locate_element(el, offset, within),
            Node::If(if_node) => {
                let mut found = None;
                for (_, body) in &if_node.branches {
                    found = found.or_else(|| locate_nodes(body, offset, within));
                }
                found.or_else(|| {
                    if_node
                        .otherwise
                        .as_deref()
                        .and_then(|body| locate_nodes(body, offset, within))
                })
            }
            Node::For(f) => locate_nodes(&f.body, offset, within),
            Node::Snippet(s) => locate_nodes(&s.body, offset, within),
            _ => None,
        };
        if hit.is_some() {
            return hit;
        }
    }
    None
}

fn locate_element(el: &Element, offset: usize, within: Option<&str>) -> Option<Target> {
    let is_component = el.kind == ElementKind::Component;

    // The tag name itself.
    if is_component && contains(el.tag.span, offset) {
        return Some(Target::ComponentName {
            name: el.tag.as_str().to_string(),
            span: el.tag.span,
        });
    }

    // Attributes: a component's own props, a slot declaration's name, and a
    // `slot="…"` fill on any element inside a component.
    for attr in &el.attrs {
        let name = attr.name.as_str();

        if el.kind == ElementKind::Slot
            && name == "name"
            && let Some((value, span)) = static_value(&attr.value)
            && contains(span, offset)
        {
            return Some(Target::SlotName {
                name: Some(value.to_string()),
                span,
            });
        }

        if name == "slot"
            && let Some((value, span)) = static_value(&attr.value)
            && contains(span, offset)
        {
            return Some(Target::SlotFill {
                component: within.map(str::to_string),
                name: value.to_string(),
                span,
            });
        }

        if is_component && contains(attr.name.span, offset) {
            return Some(Target::ComponentAttr {
                component: el.tag.as_str().to_string(),
                attr: name.to_string(),
                span: attr.name.span,
            });
        }
    }

    // A bare `<slot/>` (no `name`) is the default slot; hovering its tag names
    // it. Checked after attributes so `<slot name="x">` reports the name.
    if el.kind == ElementKind::Slot && contains(el.tag.span, offset) {
        return Some(Target::SlotName {
            name: el
                .attrs
                .iter()
                .find(|a| a.name.as_str() == "name")
                .and_then(|a| static_value(&a.value))
                .map(|(v, _)| v.to_string()),
            span: el.tag.span,
        });
    }

    // Descend. A component element becomes the ancestor for its children, so a
    // `slot="…"` on a child resolves to it.
    let child_within = if is_component {
        Some(el.tag.as_str())
    } else {
        within
    };
    locate_nodes(&el.children, offset, child_within)
}

/// The text and span of an attribute value that is a single static string —
/// `name="footer"`. `None` for an interpolating, expression, or boolean value.
fn static_value(value: &AttrValue) -> Option<(&str, Span)> {
    match value {
        AttrValue::Literal(parts) => match parts.as_slice() {
            [damask_template::AttrPart::Text(t)] => Some((t.as_str(), t.span)),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `offset` falls within `span`, inclusive of the end so a cursor
/// resting just past the last byte of a token still counts as on it.
fn contains(span: Span, offset: usize) -> bool {
    offset >= span.start && offset <= span.end
}

#[cfg(test)]
mod tests {
    use super::*;
    use damask_template::parse;

    fn at(src: &str, needle: &str) -> Option<Target> {
        let offset = src.find(needle).unwrap();
        locate(&parse(src).unwrap(), offset + 1)
    }

    #[test]
    fn component_name() {
        let t = at("<Frame title={x}>hi</Frame>", "Frame");
        assert_eq!(
            t,
            Some(Target::ComponentName {
                name: "Frame".into(),
                span: Span::new(1, 6),
            })
        );
    }

    #[test]
    fn component_attribute() {
        let Some(Target::ComponentAttr {
            component, attr, ..
        }) = at("<Frame title={x}/>", "title")
        else {
            panic!("expected a component attribute");
        };
        assert_eq!(component, "Frame");
        assert_eq!(attr, "title");
    }

    #[test]
    fn html_attribute_is_not_a_target() {
        // A lowercase element's attribute is not a component prop.
        assert_eq!(at("<div class=\"a\"/>", "class"), None);
    }

    #[test]
    fn slot_declaration() {
        let Some(Target::SlotName { name, .. }) = at(r#"<slot name="footer">x</slot>"#, "footer")
        else {
            panic!("expected a slot declaration");
        };
        assert_eq!(name.as_deref(), Some("footer"));
    }

    #[test]
    fn default_slot_declaration() {
        let Some(Target::SlotName { name, .. }) = at("<div><slot/></div>", "slot") else {
            panic!("expected the default slot");
        };
        assert_eq!(name, None);
    }

    #[test]
    fn slot_fill_resolves_enclosing_component() {
        let Some(Target::SlotFill {
            component, name, ..
        }) = at(
            r#"<Frame><span slot="footer">c</span></Frame>"#,
            r#"footer"#,
        )
        else {
            panic!("expected a slot fill");
        };
        assert_eq!(component.as_deref(), Some("Frame"));
        assert_eq!(name, "footer");
    }

    #[test]
    fn locates_through_control_flow() {
        let Some(Target::ComponentAttr { attr, .. }) =
            at("{#if self.ok}<Card title={x}/>{/if}", "title")
        else {
            panic!("expected a component attribute inside an if");
        };
        assert_eq!(attr, "title");
    }

    #[test]
    fn plain_text_is_none() {
        assert_eq!(at("just some text", "text"), None);
    }
}
