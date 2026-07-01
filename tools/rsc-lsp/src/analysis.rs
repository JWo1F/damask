//! Cursor-context analysis over template text.
//!
//! Deliberately tolerant of half-typed tags: completion must work while the user
//! is in the middle of writing `{ self.`, when a full parse would fail.

pub use rsc_template::in_tag as in_code_tag;

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
    fn detects_open_tag() {
        let t = "Hello { self.name";
        assert!(in_code_tag(t, t.len()));
    }

    #[test]
    fn closed_tag_is_not_in_a_tag() {
        let t = "Hello { self.name } world";
        assert!(!in_code_tag(t, t.len()));
    }

    #[test]
    fn text_outside_a_tag() {
        assert!(!in_code_tag("just text", 9));
    }

    #[test]
    fn nested_braces_stay_in_tag() {
        let t = "{@render Card { title: ";
        assert!(in_code_tag(t, t.len()));
    }

    #[test]
    fn self_access_forms() {
        assert!(is_self_access("{ self."));
        assert!(is_self_access("{ self.na"));
        assert!(is_self_access("{#if self.admin} { self."));
        assert!(!is_self_access("{ self"));
        assert!(!is_self_access("{ other."));
        assert!(!is_self_access("{#each xs as x"));
    }
}
