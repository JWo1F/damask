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

/// What a renderer does with the newlines in the markup passing through it.
///
/// Every variant produces the same document. HTML collapses a run of
/// whitespace containing a newline to a single space wherever whitespace is
/// insignificant, so *resizing* such a run — padding it out to indent, or
/// crushing it to one space — cannot change what a browser draws. None of these
/// ever add a newline where the template had none, or remove one that separated
/// two things the author wrote on separate lines; creating or destroying a
/// newline is the operation that would change rendering, and no variant does
/// it. Regions marked verbatim are exempt from all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    /// Emit exactly the bytes the templates hold. Each component's markup is
    /// laid out from its own root, so nesting restarts at column 0 wherever one
    /// is rendered inside another.
    AsWritten,
    /// Indent each component's markup to the depth of the call site that
    /// rendered it, so the document reads as one tree.
    Pretty,
    /// Replace each newline and the indentation after it with the single space
    /// it already renders as.
    Minified,
}

impl Default for Whitespace {
    /// The crate's `pretty` / `minify` features, which is how a binary picks a
    /// policy for every component at once. `minify` wins when both are on:
    /// cargo features are additive, so one dependency asking for `pretty` must
    /// not be able to un-minify another's release build.
    fn default() -> Self {
        if cfg!(feature = "minify") {
            Whitespace::Minified
        } else if cfg!(feature = "pretty") {
            Whitespace::Pretty
        } else {
            Whitespace::AsWritten
        }
    }
}

/// A `Renderer` that accumulates into a `String` using a configurable escape
/// policy. The built-in renderers are thin newtypes over this; a custom
/// renderer can reuse it or replace it wholesale.
#[derive(Debug, Clone)]
pub struct StringRenderer {
    buf: String,
    escape: EscapeFn,
    whitespace: Whitespace,
    /// Levels of nesting opened by the call sites above the markup being
    /// written, in [`Whitespace::Pretty`].
    indent: usize,
    /// Depth of enclosing verbatim elements. A counter rather than a flag
    /// because `<pre>` can contain a component containing another.
    verbatim: usize,
    /// Whether the buffer currently ends with a newline and nothing but
    /// indentation, so a run arriving now would be a second separator.
    ///
    /// The lowerer removes these within a template, but it cannot see across a
    /// component boundary: the caller's markup and the component's are compiled
    /// apart, and each ends and begins with its own newline. Only the thing
    /// holding the buffer knows they met.
    line_open: bool,
}

/// Spaces per level of nesting.
const INDENT_WIDTH: usize = 2;

impl StringRenderer {
    /// Create a renderer with the given escape policy.
    pub fn with_escape(escape: EscapeFn) -> Self {
        StringRenderer {
            buf: String::new(),
            escape,
            whitespace: Whitespace::default(),
            indent: 0,
            verbatim: 0,
            line_open: false,
        }
    }

    /// Set how newlines in the markup are treated.
    pub fn with_whitespace(mut self, whitespace: Whitespace) -> Self {
        self.whitespace = whitespace;
        self
    }

    /// Append markup, applying the whitespace policy to the runs that follow
    /// its newlines.
    ///
    /// The scan is skipped entirely in [`Whitespace::AsWritten`] and inside a
    /// verbatim region, which keeps the default path a single `push_str`.
    fn write_markup(&mut self, s: &str) {
        if self.whitespace == Whitespace::AsWritten || self.verbatim > 0 {
            self.buf.push_str(s);
            self.line_open = false;
            return;
        }
        const WS: [char; 4] = [' ', '\t', '\r', '\n'];
        let mut rest = s;
        let mut wrote_separator = false;

        // Whitespace arriving on an already-open line is the duplicate left
        // where two separately-compiled templates met: the caller's separator
        // and the component's own leading indent are one gap, and the separator
        // for it has been written. Plain spaces count — a template beginning
        // `··<div>` contributes them with no newline of its own.
        if self.line_open {
            rest = rest.trim_start_matches(WS);
        }
        while let Some(nl) = rest.find('\n') {
            // The run is every whitespace byte around the newline, not just the
            // newline: several in a row are one separator and must produce one
            // result, or a blank line would emit two spaces where it renders as
            // one.
            let head = &rest[..nl];
            self.buf.push_str(head.trim_end_matches(WS));
            let run_end = nl + rest[nl..].len() - rest[nl..].trim_start_matches(WS).len();
            let run = &rest[nl..run_end];
            rest = &rest[run_end..];

            match self.whitespace {
                // The template's own indentation is its depth *within* its
                // component; what is added here is the depth of the call sites
                // above it. Both are indentation of the same line, so they sum.
                Whitespace::Pretty => {
                    self.buf.push('\n');
                    let own = run.rsplit('\n').next().unwrap_or("");
                    for _ in 0..self.indent * INDENT_WIDTH {
                        self.buf.push(' ');
                    }
                    self.buf.push_str(own);
                }
                Whitespace::Minified => self.buf.push(' '),
                Whitespace::AsWritten => unreachable!("handled above"),
            }
            wrote_separator = true;
        }
        self.buf.push_str(rest);
        // The loop consumes each run whole, so `rest` either is empty or starts
        // with content: the line is still open exactly when nothing followed the
        // separator this call wrote, or a previous call left it open and this
        // one wrote nothing at all.
        self.line_open = rest.is_empty() && (wrote_separator || self.line_open);
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
        self.line_open = false;
    }

    fn write_text(&mut self, s: &str) {
        self.write_markup(s);
    }

    /// Escaped values are *data*, not markup, so they are written through
    /// untouched: a newline inside a hostname or a log line is the value's, and
    /// laying it out would be this renderer editing content rather than
    /// formatting a document.
    fn write_escaped(&mut self, value: &dyn Display) {
        let mut sink = EscapeSink {
            out: &mut self.buf,
            escape: self.escape,
        };
        // Writing to a String via a sink is infallible.
        let _ = write!(sink, "{value}");
        self.line_open = false;
    }

    /// `{@html …}` splices a value in whole. Like an escaped one, its newlines
    /// are the value's own and not this renderer's to move.
    fn write_display_raw(&mut self, value: &dyn Display) {
        let _ = write!(self.buf, "{value}");
        self.line_open = false;
    }

    /// Rewrites the indentation standing at the end of the buffer. Only in
    /// [`Whitespace::Pretty`], which is the only policy that writes any:
    /// minified output has a single space there, and it is already right.
    fn close_line(&mut self, depth: usize) {
        if self.whitespace != Whitespace::Pretty || self.verbatim > 0 || !self.line_open {
            return;
        }
        self.buf.truncate(self.buf.trim_end_matches([' ', '\t']).len());
        for _ in 0..(self.indent + depth) * INDENT_WIDTH {
            self.buf.push(' ');
        }
    }

    fn push_indent(&mut self, levels: usize) {
        self.indent += levels;
    }

    fn pop_indent(&mut self, levels: usize) {
        self.indent = self.indent.saturating_sub(levels);
    }

    fn set_verbatim(&mut self, on: bool) {
        if on {
            self.verbatim += 1;
        } else {
            self.verbatim = self.verbatim.saturating_sub(1);
        }
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
            /// Create an empty renderer that emits templates as written.
            pub fn new() -> Self {
                $name(StringRenderer::with_escape($escape))
            }

            /// Create an empty renderer with the given [`Whitespace`].
            pub fn with_whitespace(whitespace: Whitespace) -> Self {
                $name(StringRenderer::with_escape($escape).with_whitespace(whitespace))
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
            fn write_text(&mut self, s: &str) {
                self.0.write_text(s);
            }
            fn write_escaped(&mut self, value: &dyn Display) {
                self.0.write_escaped(value);
            }
            fn write_display_raw(&mut self, value: &dyn Display) {
                self.0.write_display_raw(value);
            }
            fn close_line(&mut self, depth: usize) {
                self.0.close_line(depth);
            }
            fn push_indent(&mut self, levels: usize) {
                self.0.push_indent(levels);
            }
            fn pop_indent(&mut self, levels: usize) {
                self.0.pop_indent(levels);
            }
            fn set_verbatim(&mut self, on: bool) {
                self.0.set_verbatim(on);
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

    // ------------------------------------------------------------ whitespace

    fn render(ws: Whitespace, f: impl FnOnce(&mut dyn Renderer)) -> String {
        let mut r = HtmlRenderer::with_whitespace(ws);
        f(&mut r);
        Box::new(r).finish()
    }

    #[test]
    fn as_written_copies_markup_byte_for_byte() {
        let out = render(Whitespace::AsWritten, |r| {
            r.push_indent(3);
            r.write_text("<a>\n  <b/>\n</a>");
        });
        assert_eq!(out, "<a>\n  <b/>\n</a>", "the default must stay a memcpy");
    }

    #[test]
    fn pretty_adds_the_call_sites_depth_to_each_line() {
        // The literal carries depth *within* its own template; the push carries
        // the depth of the call sites above it. The output is the sum.
        let out = render(Whitespace::Pretty, |r| {
            r.push_indent(2);
            r.write_text("<a>\n  <b/>\n</a>");
        });
        assert_eq!(out, "<a>\n      <b/>\n    </a>");
    }

    #[test]
    fn pretty_leaves_the_root_untouched() {
        let out = render(Whitespace::Pretty, |r| r.write_text("<a>\n  <b/>\n</a>"));
        assert_eq!(out, "<a>\n  <b/>\n</a>");
    }

    #[test]
    fn popping_returns_to_the_enclosing_depth() {
        let out = render(Whitespace::Pretty, |r| {
            r.push_indent(1);
            r.pop_indent(1);
            r.write_text("x\ny");
        });
        assert_eq!(out, "x\ny");
    }

    #[test]
    fn minified_replaces_each_newline_run_with_the_space_it_renders_as() {
        let out = render(Whitespace::Minified, |r| r.write_text("<a>\n\n    <b/>\n</a>"));
        assert_eq!(out, "<a> <b/> </a>");
    }

    /// The one thing that would change the document: two elements the author
    /// wrote adjacent must not acquire a space between them.
    #[test]
    fn minified_never_separates_what_was_adjacent() {
        let out = render(Whitespace::Minified, |r| r.write_text("<b>a</b><b>c</b>"));
        assert_eq!(out, "<b>a</b><b>c</b>");
    }

    /// And the converse: a space the author wrote inside a line is content, not
    /// layout, whichever policy is running.
    #[test]
    fn a_space_within_a_line_survives_every_policy() {
        for ws in [Whitespace::AsWritten, Whitespace::Pretty, Whitespace::Minified] {
            let out = render(ws, |r| r.write_text("<b>6 Mbps</b> up"));
            assert_eq!(out, "<b>6 Mbps</b> up", "{ws:?}");
        }
    }

    #[test]
    fn verbatim_regions_are_left_alone() {
        for ws in [Whitespace::Pretty, Whitespace::Minified] {
            let out = render(ws, |r| {
                r.push_indent(4);
                r.set_verbatim(true);
                r.write_text("line one\n  line two");
                r.set_verbatim(false);
            });
            assert_eq!(out, "line one\n  line two", "{ws:?}");
        }
    }

    /// `<pre>` can contain a component containing another, so leaving the outer
    /// one must not re-enable layout inside it.
    #[test]
    fn verbatim_nests() {
        let out = render(Whitespace::Minified, |r| {
            r.set_verbatim(true);
            r.set_verbatim(true);
            r.set_verbatim(false);
            r.write_text("a\nb");
        });
        assert_eq!(out, "a\nb");
    }

    /// An escaped value is data. A newline inside a hostname or a log line
    /// belongs to the value, and laying it out would be editing content.
    #[test]
    fn escaped_values_are_never_laid_out() {
        let out = render(Whitespace::Minified, |r| {
            r.push_indent(2);
            r.write_escaped(&"one\ntwo");
        });
        assert_eq!(out, "one\ntwo");
    }

    /// `{@html …}` splices a value whole, so it is not.
    #[test]
    fn raw_display_output_is_not_laid_out() {
        let out = render(Whitespace::Minified, |r| r.write_display_raw(&"<i>a</i>\n  <i>b</i>"));
        assert_eq!(out, "<i>a</i>\n  <i>b</i>");
    }

    /// The separator before an end tag was written for the last child, which
    /// puts it a level too deep. `close_line` is what an element uses to say
    /// where its own tag belongs.
    #[test]
    fn closing_a_line_re_indents_to_the_elements_own_depth() {
        let out = render(Whitespace::Pretty, |r| {
            r.write_raw("<a>");
            r.write_text("\n    ");
            r.write_raw("<b></b>");
            r.write_text("\n    ");
            r.close_line(0);
            r.write_raw("</a>");
        });
        assert_eq!(out, "<a>\n    <b></b>\n</a>");
    }

    #[test]
    fn closing_a_line_counts_the_call_sites_depth_too() {
        let out = render(Whitespace::Pretty, |r| {
            r.push_indent(2);
            r.write_text("\n      ");
            r.close_line(1);
            r.write_raw("</a>");
        });
        assert_eq!(out, "\n      </a>", "2 pushed + 1 own = 3 levels");
    }

    /// The one case it must decline: content that was written on a single line
    /// has no separator to correct, and inventing one would put a space inside
    /// an inline run.
    #[test]
    fn closing_a_line_never_breaks_inline_content() {
        let out = render(Whitespace::Pretty, |r| {
            r.push_indent(3);
            r.write_raw("<span>");
            r.write_text("Wi-Fi");
            r.close_line(0);
            r.write_raw("</span>");
        });
        assert_eq!(out, "<span>Wi-Fi</span>");
    }

    #[test]
    fn closing_a_line_leaves_minified_output_alone() {
        let out = render(Whitespace::Minified, |r| {
            r.write_raw("<a>");
            r.write_text("\n  ");
            r.close_line(0);
            r.write_raw("</a>");
        });
        assert_eq!(out, "<a> </a>");
    }

    #[test]
    fn closing_a_line_is_inert_inside_a_verbatim_region() {
        let out = render(Whitespace::Pretty, |r| {
            r.set_verbatim(true);
            r.write_text("\n      ");
            r.close_line(0);
            r.write_raw("</pre>");
        });
        assert_eq!(out, "\n      </pre>");
    }

    /// A tag is written raw, because the only newline one can hold is inside an
    /// attribute value — and that value is content.
    #[test]
    fn a_multi_line_attribute_is_never_re_indented() {
        let out = render(Whitespace::Pretty, |r| {
            r.push_indent(3);
            r.write_raw("<p title=\"one\n  two\">");
        });
        assert_eq!(out, "<p title=\"one\n  two\">");
    }

    #[test]
    fn minified_leaves_an_attribute_value_alone() {
        let out = render(Whitespace::Minified, |r| r.write_raw("<p title=\"one\n  two\">"));
        assert_eq!(out, "<p title=\"one\n  two\">");
    }
}
