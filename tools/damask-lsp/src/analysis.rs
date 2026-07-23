//! Cursor-context analysis over template text.
//!
//! Deliberately tolerant of half-typed tags: completion must work while the user
//! is in the middle of writing `{ self.`, `<Fra`, `<Frame ti`, or `{use cr`.

use damask_template::in_tag;

/// What the cursor is positioned to complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// Inside a `{ … }` tag — complete `self` members.
    SelfMember,
    /// Inside a `{use …}` tag — complete component paths.
    UsePath,
    /// Typing an element name after `<` — complete component names.
    ElementName,
    /// Inside `<Component …` attribute position — complete the component's fields.
    Attribute(String),
    /// Nowhere useful.
    None,
}

/// Classify what the cursor at `offset` should complete.
pub fn cursor_context(text: &str, offset: usize) -> Context {
    let offset = offset.min(text.len());
    // A string is text whatever encloses it — a Rust literal, or the class name
    // that keys a conditional map. Neither wants `self` members offered inside
    // it, and the map's key is not Rust at all.
    if in_string(text, offset) {
        return Context::None;
    }
    if in_tag(text, offset) {
        return if is_use_tag(text, offset) {
            Context::UsePath
        } else {
            Context::SelfMember
        };
    }
    // A `class=[…]` list holds Rust expressions but no braces, so nothing above
    // recognises it — which is why completion used to stop at the bracket.
    if in_class_list(text, offset) {
        return Context::SelfMember;
    }
    match enclosing_open_element(&text[..offset]) {
        Some((_, true)) => Context::ElementName,
        Some((name, false)) if name.chars().next().is_some_and(char::is_uppercase) => {
            Context::Attribute(name)
        }
        _ => Context::None,
    }
}

/// Whether `offset` sits inside a double-quoted string.
///
/// Scans from the start so an apostrophe in prose ("don't") cannot open one:
/// quotes are only counted once something has been entered — a tag, or a class
/// list — that can hold a string in the first place.
fn in_string(text: &str, offset: usize) -> bool {
    let bytes = text.as_bytes();
    let (mut i, mut depth, mut open) = (0usize, 0i32, false);
    while i < offset {
        match bytes[i] {
            b'{' if !open => depth += 1,
            b'}' if !open => depth = (depth - 1).max(0),
            b'[' if !open => depth += 1,
            b']' if !open => depth = (depth - 1).max(0),
            b'"' if depth > 0 => open = !open,
            b'\\' if open => i += 1,
            _ => {}
        }
        i += 1;
    }
    open
}

/// Whether the cursor sits inside an unclosed `class=[ … ]` list.
fn in_class_list(text: &str, offset: usize) -> bool {
    let before = &text[..offset];
    let Some(open) = before.rfind('[') else {
        return false;
    };
    if before[open..].contains(']') {
        return false;
    }
    // The `[` has to be this attribute's value, not a bracket inside some other
    // expression, so what precedes it must be `class=`.
    let head = before[..open].trim_end();
    if !head.ends_with('=') {
        return false;
    }
    head[..head.len() - 1].trim_end().ends_with("class")
}

/// Whether the tag enclosing the cursor is a `{use …}` statement.
fn is_use_tag(text: &str, offset: usize) -> bool {
    let before = &text[..offset];
    let mut depth = 0;
    for (i, c) in before.char_indices().rev() {
        match c {
            '}' => depth += 1,
            '{' => {
                if depth == 0 {
                    let after = text[i + 1..offset].trim_start();
                    return after
                        .strip_prefix("use")
                        .is_some_and(|r| r.is_empty() || r.starts_with(char::is_whitespace));
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    false
}

/// If the cursor sits inside an unclosed `<name …` tag, return `(name,
/// still_typing_name)`.
fn enclosing_open_element(before: &str) -> Option<(String, bool)> {
    let lt = before.rfind('<')?;
    if before[lt..].contains('>') {
        return None; // the tag is already closed
    }
    let after = &before[lt + 1..];
    if after.starts_with('/') {
        return None; // a closing tag
    }
    let name: String = after
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    let still_typing_name = after.len() == name.len();
    Some((name, still_typing_name))
}

/// If the cursor sits inside a `slot="…"` attribute value, return the nearest
/// enclosing component element — the one whose slot is being filled. Tolerant of
/// the half-typed, not-yet-closed tag the cursor is in, so completion fires while
/// the value is still being written.
pub fn slot_fill_component(text: &str, offset: usize) -> Option<String> {
    let offset = offset.min(text.len());
    let before = &text[..offset];

    // The cursor must be inside the value of an unclosed tag's `slot` attribute.
    let lt = before.rfind('<')?;
    let tag = &before[lt..];
    if tag.contains('>') {
        return None; // the tag is already closed — not in its attributes
    }
    let eq = tag.rfind('=')?;
    // The attribute name is the word ending at `=`.
    let name = tag[..eq]
        .trim_end()
        .rsplit(|c: char| c.is_whitespace())
        .next()?;
    if name != "slot" {
        return None;
    }
    // The value after `=` must be an open quote (no closing one yet).
    let value = tag[eq + 1..].trim_start();
    let quote = value.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    if value[quote.len_utf8()..].contains(quote) {
        return None; // the value is already closed
    }

    nearest_component_ancestor(&text[..lt])
}

/// If the cursor is in attribute-*name* position of an element nested inside a
/// component, return that component — so `slot` can be offered as the attribute
/// that fills one of its slots. `None` inside a value (that is a slot *value*
/// position, handled by [`slot_fill_component`]) or with no component ancestor.
pub fn slot_attribute_component(text: &str, offset: usize) -> Option<String> {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let lt = before.rfind('<')?;
    let tag = &before[lt..];
    if tag.contains('>') || tag.starts_with("</") {
        return None;
    }
    // Must be past the element name — some whitespace separates it from attrs.
    let after_name = tag[1..]
        .trim_start_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '-' || c == ':');
    if after_name.is_empty() {
        return None; // still typing the element name
    }
    // Not inside a quoted value: an odd number of quotes means one is open.
    if tag.matches('"').count() % 2 == 1 || tag.matches('\'').count() % 2 == 1 {
        return None;
    }
    nearest_component_ancestor(&text[..lt])
}

/// The nearest still-open component (capitalized) element enclosing the end of
/// `before`, by walking its element tags into a stack.
fn nearest_component_ancestor(before: &str) -> Option<String> {
    let mut stack: Vec<String> = Vec::new();
    let bytes = before.as_bytes();
    let mut i = 0;
    while i < before.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &before[i + 1..];
        // `<!-- … -->` / `<!…>` are not elements.
        if rest.starts_with('!') {
            i += rest.find('>').map(|g| g + 2).unwrap_or(before.len() - i);
            continue;
        }
        if let Some(after) = rest.strip_prefix('/') {
            // A closing tag pops the matching open element.
            let name: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if let Some(top) = stack.last()
                && *top == name
            {
                stack.pop();
            } else if !stack.is_empty() {
                stack.pop();
            }
            i += rest.find('>').map(|g| g + 2).unwrap_or(before.len() - i);
            continue;
        }
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if name.is_empty() {
            i += 1; // a `<` that is not a tag (prose), handled as text
            continue;
        }
        // Advance to the tag's `>`; a `/>` just before it is self-closing and
        // opens no scope.
        let Some(gt) = rest.find('>') else {
            break; // an unclosed tag — the element the cursor is inside
        };
        let self_closing = rest[..gt].trim_end().ends_with('/');
        if !self_closing {
            stack.push(name);
        }
        i += gt + 2;
    }
    stack
        .into_iter()
        .rev()
        .find(|n| n.chars().next().is_some_and(char::is_uppercase))
}

/// Whether the text immediately before the cursor is a `self.` member access.
pub fn is_self_access(before: &str) -> bool {
    let trimmed = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    trimmed.ends_with("self.")
}

// Re-export for existing callers.
pub use damask_template::in_tag as in_code_tag;

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(text: &str) -> Context {
        cursor_context(text, text.len())
    }

    #[test]
    fn self_member_context() {
        assert_eq!(ctx("Hi { self.na"), Context::SelfMember);
        assert_eq!(ctx("<Foo a={ self."), Context::SelfMember);
    }

    #[test]
    fn use_context() {
        assert_eq!(ctx("<div>{use crate::wid"), Context::UsePath);
    }

    #[test]
    fn element_name_context() {
        assert_eq!(ctx("hi <"), Context::ElementName);
        assert_eq!(ctx("hi <Fra"), Context::ElementName);
    }

    #[test]
    fn attribute_context() {
        assert_eq!(ctx("<Frame ti"), Context::Attribute("Frame".into()));
        assert_eq!(
            ctx("<Frame title={x} on"),
            Context::Attribute("Frame".into())
        );
        // lowercase (HTML) element -> no field source
        assert_eq!(ctx("<div cla"), Context::None);
    }

    #[test]
    fn closed_tag_is_none() {
        assert_eq!(ctx("<Frame title={x}>text"), Context::None);
    }

    #[test]
    fn class_list_entries_complete_as_rust() {
        // The list holds Rust expressions but no braces, so nothing else would
        // recognise the position.
        assert_eq!(ctx(r#"<div class=[self."#), Context::SelfMember);
        assert_eq!(ctx(r#"<div class=[a, self.x"#), Context::SelfMember);
        // Closed again: back to the attribute position of a plain element.
        assert_eq!(ctx(r#"<div class=[a] "#), Context::None);
        // A bracket that is not a class value stays what it was.
        assert_eq!(ctx(r#"<div other=[self."#), Context::None);
    }

    #[test]
    fn a_class_maps_key_is_not_rust() {
        // Inside the key's quotes: a class name, so no `self` members.
        assert_eq!(ctx(r#"<div class={ "px-"#), Context::None);
        // The condition after it is Rust again.
        assert_eq!(ctx(r#"<div class={ "px-3": self."#), Context::SelfMember);
    }

    #[test]
    fn a_comment_completes_nothing() {
        // The braces in a sentence are prose, not a tag.
        assert_eq!(ctx("{# a note about self. and {braces} "), Context::None);
        // ...and the tag after a closed comment still works.
        assert_eq!(ctx("{# a note #}{ self."), Context::SelfMember);
    }

    #[test]
    fn self_access_forms() {
        assert!(is_self_access("{ self."));
        assert!(!is_self_access("{ other."));
    }

    fn slot_at_end(text: &str) -> Option<String> {
        slot_fill_component(text, text.len())
    }

    #[test]
    fn slot_value_resolves_component() {
        assert_eq!(slot_at_end(r#"<Frame><span slot=""#), Some("Frame".into()));
        assert_eq!(
            slot_at_end(r#"<Frame><span slot="foo"#),
            Some("Frame".into())
        );
        // Single quotes too.
        assert_eq!(slot_at_end(r#"<Frame><span slot='"#), Some("Frame".into()));
    }

    #[test]
    fn slot_value_skips_closed_siblings_and_self_closing() {
        // A closed sibling element must not be treated as the ancestor.
        assert_eq!(
            slot_at_end(r#"<Frame><img/><span slot=""#),
            Some("Frame".into())
        );
        // Nested components: the nearest one wins.
        assert_eq!(
            slot_at_end(r#"<Outer><Inner><span slot=""#),
            Some("Inner".into())
        );
    }

    #[test]
    fn slot_value_needs_a_component_ancestor() {
        // A lowercase-only ancestor is not a component.
        assert_eq!(slot_at_end(r#"<div><span slot=""#), None);
        // Not a `slot` attribute.
        assert_eq!(slot_at_end(r#"<Frame><span class=""#), None);
        // The value is already closed.
        assert_eq!(slot_at_end(r#"<Frame><span slot="a" "#), None);
    }

    fn slot_attr_at_end(text: &str) -> Option<String> {
        slot_attribute_component(text, text.len())
    }

    #[test]
    fn slot_attribute_offered_on_component_child() {
        // In attribute-name position on a child of a component.
        assert_eq!(slot_attr_at_end("<Frame><span "), Some("Frame".into()));
        assert_eq!(slot_attr_at_end("<Frame><span sl"), Some("Frame".into()));
        // Still typing the element name — not yet an attribute position.
        assert_eq!(slot_attr_at_end("<Frame><spa"), None);
        // Inside a value is a slot-*value* position, not a name one.
        assert_eq!(slot_attr_at_end(r#"<Frame><span slot=""#), None);
        // No component ancestor.
        assert_eq!(slot_attr_at_end("<div><span "), None);
    }
}
