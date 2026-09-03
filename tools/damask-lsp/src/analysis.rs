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
    // A string is text whatever encloses it — a Rust literal, or the name that
    // keys an `@tokens` or `@attrs` entry. Neither wants `self` members offered
    // inside it, and a key is not Rust at all.
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

/// The two attribute helpers, with the one-line summary a completion shows and
/// the longer text a hover does.
pub const HELPERS: [Helper; 2] = [
    Helper {
        name: "tokens",
        detail: "a space-separated value",
        doc: "`{@tokens(…)}` builds one space-separated value — a `class`, a `rel`, a \
              `sandbox`.\n\n\
              ```html\n\
              <div class={@tokens(self.extra, \"base\", \"is-open\": self.open)}>\n\
              ```\n\n\
              A positional entry is a name, a list of them, or an `Option` of either; a \
              literal `None` is dropped when the template compiles. A `name: cond` entry is \
              there while `cond` holds, and the name is a bare identifier or a string for \
              anything an identifier cannot spell (`\"md:px-3\": cond`).\n\n\
              Names are deduplicated, keep their first mention's order, and an empty result \
              writes no attribute at all. On a component prop the helper yields a `String`.",
    },
    Helper {
        name: "attrs",
        detail: "a run of `<name>-*` attributes",
        doc: "`{@attrs(…)}` expands into a run of attributes under the name it is written \
              on — `data-*`, `aria-*`, whatever the attribute is called.\n\n\
              ```html\n\
              <div data={@attrs(self.hooks(), controller: \"modal\", index: self.i)}>\n\
              ```\n\n\
              A positional entry is anything implementing `AttrSet` — a pair list, a \
              `HashMap`, a `BTreeMap`, an `Attrs`, an `Option` of any of them — and a later \
              entry overrides an earlier one, keeping the first mention's position. A \
              `key: value` entry writes one attribute; the key is taken verbatim, so \
              `user_id` is `data-user_id`.\n\n\
              Values follow the `Attr` rules one level down: a `bool` writes a bare \
              attribute or none, an `Option` writes nothing when `None`.\n\n\
              It writes attribute *names*, so it belongs on an element; a set that a \
              component cannot name reaches it through `{...expr}`.",
    },
];

/// One attribute helper, as the editor describes it.
pub struct Helper {
    pub name: &'static str,
    pub detail: &'static str,
    pub doc: &'static str,
}

/// The helper the cursor is on — `@tokens` or `@attrs` — with the span of the
/// `@name` it was written as, for a hover to underline.
pub fn helper_at(text: &str, offset: usize) -> Option<(&'static Helper, usize, usize)> {
    let offset = offset.min(text.len());
    let start = text[..offset]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphabetic())
        .last()
        .map(|(i, _)| i)
        .unwrap_or(offset);
    let end = offset
        + text[offset..]
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_alphabetic())
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .unwrap_or(0);
    // The `@` is part of the name as far as a reader is concerned, and it is
    // what tells a helper from a field or a local called `attrs`.
    let at = start.checked_sub(1)?;
    if text.as_bytes().get(at) != Some(&b'@') {
        return None;
    }
    let name = &text[start..end];
    HELPERS
        .iter()
        .find(|helper| helper.name == name)
        .map(|helper| (helper, at, end))
}

/// Whether a helper is what the cursor is positioned to write: inside an
/// attribute's `{ … }`, with nothing but a partial `@name` typed so far.
///
/// The tell is the `=` before the brace. A `{ … }` in element content, or one
/// holding an expression already, is Rust and belongs to rust-analyzer.
pub fn helper_prefix(text: &str, offset: usize) -> Option<&str> {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let open = before.rfind('{')?;
    if before[open..].contains('}') {
        return None;
    }
    if !before[..open].trim_end().ends_with('=') {
        return None;
    }
    let typed = before[open + 1..].trim_start();
    let name = typed.strip_prefix('@').unwrap_or(typed);
    // Only while it could still become one: a `(` means the call is written and
    // what follows it is its arguments.
    match name.chars().all(|c| c.is_ascii_alphabetic()) {
        true => Some(typed),
        false => None,
    }
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

    /// A helper's arguments are Rust, and they are inside the `{ … }` every
    /// other Rust position is inside — so the same brace counting finds them,
    /// on `class`, on `data`, on anything.
    #[test]
    fn helper_entries_complete_as_rust() {
        assert_eq!(ctx(r#"<div class={@tokens(self."#), Context::SelfMember);
        assert_eq!(
            ctx(r#"<div class={@tokens("a", self.x"#),
            Context::SelfMember
        );
        assert_eq!(ctx(r#"<div rel={@tokens(self."#), Context::SelfMember);
        assert_eq!(ctx(r#"<div data={@attrs(self."#), Context::SelfMember);
        // Closed again: back to the attribute position of a plain element.
        assert_eq!(ctx(r#"<div class={@tokens("a")} "#), Context::None);
    }

    #[test]
    fn a_helper_is_offered_where_one_can_be_written() {
        // An attribute value, nothing typed: both helpers are candidates.
        assert_eq!(helper_prefix(r#"<div class={"#, 12), Some(""));
        assert_eq!(helper_prefix(r#"<div class={@"#, 13), Some("@"));
        assert_eq!(helper_prefix(r#"<div class={@tok"#, 16), Some("@tok"));
        // Not once the call is open — those are its arguments.
        assert_eq!(helper_prefix(r#"<div class={@tokens("#, 20), None);
        // Not in element content, and not in an expression already under way.
        assert_eq!(helper_prefix(r#"<p>{self."#, 9), None);
        assert_eq!(helper_prefix(r#"<div class={self."#, 17), None);
    }

    #[test]
    fn a_helper_is_recognised_for_hover() {
        let text = r#"<div class={@tokens("a")}>"#;
        let at = text.find("tokens").unwrap();
        let (helper, start, end) = helper_at(text, at + 2).expect("a helper");
        assert_eq!(helper.name, "tokens");
        assert_eq!(&text[start..end], "@tokens");
        // A word that only looks like one.
        assert!(helper_at("{ self.attrs }", 9).is_none());
    }

    #[test]
    fn a_helper_key_is_not_rust() {
        // Inside the key's quotes: a name, so no `self` members.
        assert_eq!(ctx(r#"<div class={@tokens("px-"#), Context::None);
        assert_eq!(ctx(r#"<div data={@attrs("cont"#), Context::None);
        // The value after it is Rust again.
        assert_eq!(
            ctx(r#"<div class={@tokens("px-3": self."#),
            Context::SelfMember
        );
        assert_eq!(
            ctx(r#"<div data={@attrs(controller: self."#),
            Context::SelfMember
        );
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
