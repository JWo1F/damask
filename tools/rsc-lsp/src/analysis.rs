//! Cursor-context analysis over template text.
//!
//! Deliberately tolerant of half-typed tags: completion must work while the user
//! is in the middle of writing `{ self.`, `<Fra`, `<Frame ti`, or `{#use cr`.

use rsc_template::in_tag;

/// What the cursor is positioned to complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// Inside a `{ … }` tag — complete `self` members.
    SelfMember,
    /// Inside a `{#use …}` tag — complete component paths.
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

/// Whether the tag enclosing the cursor is a `{#use …}`.
fn is_use_tag(text: &str, offset: usize) -> bool {
    let before = &text[..offset];
    let mut depth = 0;
    for (i, c) in before.char_indices().rev() {
        match c {
            '}' => depth += 1,
            '{' => {
                if depth == 0 {
                    return text[i + 1..offset].trim_start().starts_with("#use");
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

/// Whether the text immediately before the cursor is a `self.` member access.
pub fn is_self_access(before: &str) -> bool {
    let trimmed = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    trimmed.ends_with("self.")
}

// Re-export for existing callers.
pub use rsc_template::in_tag as in_code_tag;

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
        assert_eq!(ctx("<div>{#use crate::wid"), Context::UsePath);
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
    fn self_access_forms() {
        assert!(is_self_access("{ self."));
        assert!(!is_self_access("{ other."));
    }
}
