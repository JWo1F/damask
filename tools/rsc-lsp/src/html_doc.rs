//! The HTML skeleton projected from a `.rsc` template for an HTML language
//! server.
//!
//! Every `{ … }` tag — interpolations, directives, control-flow markers, and
//! attribute expression values — is blanked to spaces, preserving byte length
//! and newlines. What's left is the template's static markup at *exactly* the
//! same offsets as the source, so the HTML server's positions and result ranges
//! map back to the `.rsc` by the identity — no source map needed.
//!
//! Because blanking can leave unbalanced tags (a `<li>` whose `{#if}` wrapper
//! vanished), the server's *diagnostics* on this skeleton are unreliable and are
//! not surfaced; only hover and completion are forwarded.

use rsc_template::tag_spans;

/// Project `rsc_src` to an HTML skeleton by blanking every `{ … }` tag. The
/// result has the same length and line structure as the input.
pub fn html_skeleton(rsc_src: &str) -> String {
    let mut bytes = rsc_src.as_bytes().to_vec();
    for span in tag_spans(rsc_src) {
        for b in &mut bytes[span.start..span.end] {
            // Keep line breaks so line/column positions stay aligned.
            if *b != b'\n' && *b != b'\r' {
                *b = b' ';
            }
        }
    }
    // Only ASCII spaces were substituted (newlines kept), so this stays valid.
    String::from_utf8(bytes).expect("blanking preserves UTF-8 validity")
}

#[cfg(test)]
mod tests {
    use super::html_skeleton;

    #[test]
    fn blanks_tags_preserving_offsets() {
        let src = "<p class=\"a\">Hi {self.name}</p>\n{#if x}<b>y</b>{/if}";
        let out = html_skeleton(src);
        assert_eq!(
            out.len(),
            src.len(),
            "length preserved for identity mapping"
        );
        assert!(
            !out.contains('{') && !out.contains('}'),
            "tags remain: {out:?}"
        );
        // Markup keeps its exact offsets.
        assert_eq!(
            out.find("<p class=").unwrap(),
            src.find("<p class=").unwrap()
        );
        assert_eq!(out.find("<b>").unwrap(), src.find("<b>").unwrap());
        // Newline preserved so line numbers stay aligned.
        assert_eq!(out.matches('\n').count(), 1);
    }

    #[test]
    fn multibyte_inside_tag_stays_valid() {
        // A multibyte char inside a blanked tag must not corrupt the output.
        let src = "x{ \"日本\" }y";
        let out = html_skeleton(src);
        assert_eq!(out.len(), src.len());
        assert!(out.starts_with('x') && out.ends_with('y'));
        assert!(!out.contains('{') && !out.contains('}'));
    }

    #[test]
    fn quoted_attribute_braces_are_kept() {
        // Braces inside a quoted attribute literal are markup, not a tag.
        let src = r#"<a title="{x}">t</a>"#;
        assert_eq!(html_skeleton(src), src);
    }
}
