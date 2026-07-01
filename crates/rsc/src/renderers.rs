//! Built-in [`Renderer`](crate::Renderer) implementations, one per host
//! language, plus [`StringRenderer`] — the shared `String`-backed core they are
//! built on and the easiest starting point for a custom renderer.

use crate::Renderer;
use std::fmt::{self, Display, Write};

/// A function that appends `input` to `out`, applying some escaping policy.
pub type EscapeFn = fn(input: &str, out: &mut String);

/// Escape policy that copies text through unchanged.
pub fn escape_none(input: &str, out: &mut String) {
    out.push_str(input);
}

/// Escape policy for HTML text/attribute context.
///
/// Replaces `& < > " '` with their entities. Runs of ordinary characters are
/// copied in bulk, so text with no special characters costs a single
/// `push_str`.
pub fn escape_html(input: &str, out: &mut String) {
    let mut last = 0;
    for (i, ch) in input.char_indices() {
        let replacement = match ch {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&#39;",
            _ => continue,
        };
        out.push_str(&input[last..i]);
        out.push_str(replacement);
        last = i + ch.len_utf8();
    }
    out.push_str(&input[last..]);
}

/// A `Renderer` that accumulates into a `String` using a configurable escape
/// policy. The built-in renderers are thin newtypes over this; a custom
/// renderer can reuse it or replace it wholesale.
#[derive(Debug, Clone)]
pub struct StringRenderer {
    buf: String,
    escape: EscapeFn,
}

impl StringRenderer {
    /// Create a renderer with the given escape policy.
    pub fn with_escape(escape: EscapeFn) -> Self {
        StringRenderer {
            buf: String::new(),
            escape,
        }
    }

    /// Consume the renderer and return the accumulated output.
    pub fn into_string(self) -> String {
        self.buf
    }

    /// Borrow the output built so far.
    pub fn as_str(&self) -> &str {
        &self.buf
    }
}

/// Formats a `Display` value straight into a `String` through an escape policy,
/// without a full intermediate allocation (each `Display` write is escaped as it
/// arrives).
struct EscapeSink<'a> {
    out: &'a mut String,
    escape: EscapeFn,
}

impl Write for EscapeSink<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        (self.escape)(s, self.out);
        Ok(())
    }
}

impl Renderer for StringRenderer {
    fn write_raw(&mut self, s: &str) {
        self.buf.push_str(s);
    }

    fn write_escaped(&mut self, value: &dyn Display) {
        let mut sink = EscapeSink {
            out: &mut self.buf,
            escape: self.escape,
        };
        // Writing to a String via a sink is infallible.
        let _ = write!(sink, "{value}");
    }

    fn write_display_raw(&mut self, value: &dyn Display) {
        let _ = write!(self.buf, "{value}");
    }

    fn finish(self: Box<Self>) -> String {
        self.buf
    }
}

/// Define a newtype built-in renderer that delegates to [`StringRenderer`].
macro_rules! builtin_renderer {
    ($(#[$doc:meta])* $name:ident, $escape:expr) => {
        $(#[$doc])*
        #[derive(Debug, Clone)]
        pub struct $name(StringRenderer);

        impl $name {
            /// Create an empty renderer.
            pub fn new() -> Self {
                $name(StringRenderer::with_escape($escape))
            }

            /// Borrow the output built so far.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl Renderer for $name {
            fn write_raw(&mut self, s: &str) {
                self.0.write_raw(s);
            }
            fn write_escaped(&mut self, value: &dyn Display) {
                self.0.write_escaped(value);
            }
            fn write_display_raw(&mut self, value: &dyn Display) {
                self.0.write_display_raw(value);
            }
            fn finish(self: Box<Self>) -> String {
                self.0.into_string()
            }
        }
    };
}

builtin_renderer! {
    /// The default renderer: `{ … }` HTML-escapes, `{@html … }` does not.
    ///
    /// RSC templates are HTML, so this is the renderer every component uses
    /// unless driven by a custom one.
    HtmlRenderer, escape_html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escapes_special_characters() {
        let mut out = String::new();
        escape_html(r#"<a href="x">Tom & Jerry's</a>"#, &mut out);
        assert_eq!(
            out,
            "&lt;a href=&quot;x&quot;&gt;Tom &amp; Jerry&#39;s&lt;/a&gt;"
        );
    }

    #[test]
    fn html_passes_plain_text_unchanged() {
        let mut out = String::new();
        escape_html("no specials here", &mut out);
        assert_eq!(out, "no specials here");
    }

    #[test]
    fn html_renderer_escapes_only_write_escaped() {
        let mut r = HtmlRenderer::new();
        r.write_raw("<b>");
        r.write_escaped(&"<script>");
        r.write_display_raw(&"</b>");
        assert_eq!(Box::new(r).finish(), "<b>&lt;script&gt;</b>");
    }

    #[test]
    fn no_escape_policy_passes_through() {
        let mut r = StringRenderer::with_escape(escape_none);
        r.write_escaped(&"<x>");
        assert_eq!(Box::new(r).finish(), "<x>");
    }

    #[test]
    fn string_renderer_is_object_safe() {
        let mut boxed: Box<dyn Renderer> = Box::new(StringRenderer::with_escape(escape_html));
        boxed.write_escaped(&"a<b");
        assert_eq!(boxed.finish(), "a&lt;b");
    }
}
