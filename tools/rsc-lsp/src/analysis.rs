//! Cursor-context analysis over template text.
//!
//! Deliberately tolerant of half-typed tags: completion must work while the user
//! is in the middle of writing `<%= self.`, when a full parse would fail.

/// Whether `offset` sits inside an open `<% … %>` tag whose body is Rust — i.e.
/// there is a `<%` before the cursor with no `%>` between it and the cursor, and
/// it is not a `<%#` comment.
pub fn in_code_tag(text: &str, offset: usize) -> bool {
    let offset = offset.min(text.len());
    let before = &text[..offset];
    let Some(open) = before.rfind("<%") else {
        return false;
    };
    if let Some(close) = before.rfind("%>")
        && close > open
    {
        return false; // the most recent tag is already closed
    }
    let sigil = text[open + 2..].chars().next().unwrap_or(' ');
    sigil != '#'
}

/// Whether the text immediately before the cursor is a `self.` member access
/// (allowing a partially typed member name after the dot).
pub fn is_self_access(before: &str) -> bool {
    let trimmed = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    trimmed.ends_with("self.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_open_code_tag() {
        let t = "Hello <%= self.name";
        assert!(in_code_tag(t, t.len()));
    }

    #[test]
    fn closed_tag_is_not_in_code() {
        let t = "Hello <%= self.name %> world";
        assert!(!in_code_tag(t, t.len()));
    }

    #[test]
    fn text_before_any_tag_is_not_in_code() {
        let t = "just text";
        assert!(!in_code_tag(t, t.len()));
    }

    #[test]
    fn comment_tag_is_not_code() {
        let t = "x <%# note ";
        assert!(!in_code_tag(t, t.len()));
    }

    #[test]
    fn self_access_forms() {
        assert!(is_self_access("<%= self."));
        assert!(is_self_access("<%= self.na"));
        assert!(is_self_access("  if self.admin { self.")); // nearest access
        assert!(!is_self_access("<%= self"));
        assert!(!is_self_access("<%= other."));
        assert!(!is_self_access("<% for x in xs"));
    }
}
