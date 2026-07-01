use crate::{Node, Span, TagKind, Template};
use std::fmt;

/// Options controlling how a template is parsed.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParseOptions {
    /// Enable whitespace-control markers: `<%_`, `-%>`, `_%>`.
    ///
    /// Off by default, so templates render exactly as written.
    pub trim: bool,
}

/// A template parse failure, with the source span it occurred at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (bytes {}..{})",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}

/// Whitespace-trimming behavior applied next to a tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trim {
    None,
    /// Strip a single following newline (`-%>`).
    Line,
    /// Strip all adjacent whitespace (`<%_`, `_%>`).
    All,
}

/// Parse a `.rsc` template into a [`Template`].
///
/// Returns the first [`ParseError`] encountered (unclosed tag). Everything else
/// — the Rust inside a tag, the host language outside one — is left untouched
/// for later stages to interpret.
pub fn parse(src: &str, opts: &ParseOptions) -> Result<Template, ParseError> {
    let bytes = src.as_bytes();
    let n = bytes.len();

    let mut nodes: Vec<Node> = Vec::new();
    let mut text_buf = String::new();
    let mut text_start = 0usize;
    let mut pending_right = Trim::None;
    let mut i = 0usize;

    while i < n {
        // Tag opener `<%` — unless it is the `<%%` literal escape.
        if bytes[i] == b'<' && i + 1 < n && bytes[i + 1] == b'%' {
            if i + 2 < n && bytes[i + 2] == b'%' {
                text_buf.push_str("<%");
                i += 3;
                continue;
            }

            flush_text(
                &mut nodes,
                &mut text_buf,
                text_start,
                i,
                &mut pending_right,
                opts.trim,
            );

            let tag = parse_tag(src, i, opts.trim)?;

            if opts.trim
                && tag.left_trim == Trim::All
                && let Some(Node::Text { text, .. }) = nodes.last_mut()
            {
                trim_end_ws(text);
                if text.is_empty() {
                    nodes.pop();
                }
            }

            pending_right = tag.right_trim;
            nodes.push(Node::Tag {
                span: tag.span,
                kind: tag.kind,
                code: tag.code,
                code_span: tag.code_span,
            });

            i = tag.end;
            text_start = i;
            continue;
        }

        // Literal `%%>` escape in text -> `%>`.
        if bytes[i] == b'%' && i + 2 < n && bytes[i + 1] == b'%' && bytes[i + 2] == b'>' {
            text_buf.push_str("%>");
            i += 3;
            continue;
        }

        // Ordinary character (advance by its full UTF-8 width).
        let ch = src[i..].chars().next().expect("valid char boundary");
        text_buf.push(ch);
        i += ch.len_utf8();
    }

    flush_text(
        &mut nodes,
        &mut text_buf,
        text_start,
        n,
        &mut pending_right,
        opts.trim,
    );

    Ok(Template { nodes })
}

/// The result of parsing a single tag.
struct TagParse {
    kind: TagKind,
    code: String,
    code_span: Span,
    span: Span,
    left_trim: Trim,
    right_trim: Trim,
    /// Byte offset just past the closing `%>`.
    end: usize,
}

/// Parse one tag beginning at `start` (which must point at the `<` of `<%`).
fn parse_tag(src: &str, start: usize, trim_enabled: bool) -> Result<TagParse, ParseError> {
    let bytes = src.as_bytes();
    let n = bytes.len();

    // Skip `<%`, then read the kind sigil.
    let mut p = start + 2;
    let mut left_trim = Trim::None;
    let kind = match bytes.get(p) {
        Some(b'=') => {
            p += 1;
            TagKind::Escaped
        }
        Some(b'-') => {
            p += 1;
            TagKind::Raw
        }
        Some(b'+') => {
            p += 1;
            TagKind::Render
        }
        Some(b'#') => {
            p += 1;
            TagKind::Comment
        }
        Some(b'_') if trim_enabled => {
            p += 1;
            left_trim = Trim::All;
            TagKind::Statement
        }
        _ => TagKind::Statement,
    };

    let code_start = p;

    // Scan to the closing `%>`.
    let mut j = code_start;
    loop {
        if j + 1 < n && bytes[j] == b'%' && bytes[j + 1] == b'>' {
            break;
        }
        if j >= n {
            return Err(ParseError {
                message: "unclosed tag: expected `%>`".to_string(),
                span: Span::new(start, n),
            });
        }
        j += 1;
    }

    // `j` is at the `%` of the closing `%>`. Look for a right-trim marker.
    let mut code_end = j;
    let mut right_trim = Trim::None;
    if trim_enabled && code_end > code_start {
        match bytes[code_end - 1] {
            b'-' => {
                right_trim = Trim::Line;
                code_end -= 1;
            }
            b'_' => {
                right_trim = Trim::All;
                code_end -= 1;
            }
            _ => {}
        }
    }

    let code_raw = &src[code_start..code_end];
    let leading_ws = code_raw.len() - code_raw.trim_start().len();
    let code_trimmed = code_raw.trim();
    let cs_start = code_start + leading_ws;
    let code_span = Span::new(cs_start, cs_start + code_trimmed.len());

    Ok(TagParse {
        kind,
        code: code_trimmed.to_string(),
        code_span,
        span: Span::new(start, j + 2),
        left_trim,
        right_trim,
        end: j + 2,
    })
}

/// Flush the accumulated text buffer as a [`Node::Text`], applying any pending
/// right-trim to its front. Empty text (including text that trims to nothing) is
/// dropped rather than emitted.
fn flush_text(
    nodes: &mut Vec<Node>,
    text_buf: &mut String,
    start: usize,
    end: usize,
    pending_right: &mut Trim,
    trim_enabled: bool,
) {
    if trim_enabled {
        match *pending_right {
            Trim::None => {}
            Trim::Line => {
                if let Some(rest) = text_buf.strip_prefix("\r\n") {
                    *text_buf = rest.to_string();
                } else if let Some(rest) = text_buf.strip_prefix('\n') {
                    *text_buf = rest.to_string();
                }
            }
            Trim::All => {
                let start_len = text_buf.trim_start().len();
                let cut = text_buf.len() - start_len;
                text_buf.drain(..cut);
            }
        }
    }
    *pending_right = Trim::None;

    if text_buf.is_empty() {
        return;
    }
    nodes.push(Node::Text {
        span: Span::new(start, end),
        text: std::mem::take(text_buf),
    });
}

fn trim_end_ws(text: &mut String) {
    let len = text.trim_end().len();
    text.truncate(len);
}

#[cfg(test)]
impl Node {
    fn span_for_test(&self) -> Span {
        match self {
            Node::Text { span, .. } => *span,
            Node::Tag { span, .. } => *span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_default(src: &str) -> Vec<Node> {
        parse(src, &ParseOptions::default()).unwrap().nodes
    }

    fn text(span: (usize, usize), s: &str) -> Node {
        Node::Text {
            span: Span::new(span.0, span.1),
            text: s.to_string(),
        }
    }

    #[test]
    fn plain_text_is_one_node() {
        assert_eq!(
            parse_default("Hello, world!"),
            vec![text((0, 13), "Hello, world!")]
        );
    }

    #[test]
    fn empty_input_is_no_nodes() {
        assert_eq!(parse_default(""), Vec::<Node>::new());
    }

    #[test]
    fn escaped_expression_tag() {
        let nodes = parse_default("Hello <%= self.name %>!");
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0], text((0, 6), "Hello "));
        match &nodes[1] {
            Node::Tag { kind, code, .. } => {
                assert_eq!(*kind, TagKind::Escaped);
                assert_eq!(code, "self.name");
            }
            other => panic!("expected tag, got {other:?}"),
        }
        assert_eq!(nodes[2], text((22, 23), "!"));
    }

    #[test]
    fn all_tag_kinds() {
        let nodes = parse_default("<%= a %><%- b %><%+ c %><% d %><%# e %>");
        let kinds: Vec<TagKind> = nodes
            .iter()
            .filter_map(|n| match n {
                Node::Tag { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                TagKind::Escaped,
                TagKind::Raw,
                TagKind::Render,
                TagKind::Statement,
                TagKind::Comment,
            ]
        );
    }

    #[test]
    fn statement_spanning_multiple_tags() {
        let src = "<% for x in xs { %><%= x %><% } %>";
        let nodes = parse_default(src);
        let codes: Vec<&str> = nodes
            .iter()
            .filter_map(|n| match n {
                Node::Tag { code, .. } => Some(code.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(codes, vec!["for x in xs {", "x", "}"]);
    }

    #[test]
    fn literal_delimiter_escapes() {
        let nodes = parse_default("a <%% b %%> c");
        assert_eq!(nodes, vec![text((0, 13), "a <% b %> c")]);
    }

    #[test]
    fn code_span_points_at_trimmed_code() {
        let src = "<%=   self.name   %>";
        let nodes = parse_default(src);
        match &nodes[0] {
            Node::Tag {
                code, code_span, ..
            } => {
                assert_eq!(code, "self.name");
                assert_eq!(code_span.slice(src), "self.name");
            }
            other => panic!("expected tag, got {other:?}"),
        }
    }

    #[test]
    fn multibyte_text_is_preserved() {
        let nodes = parse_default("héllo <%= x %> 😀");
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[2], text((15, 20), " 😀"));
    }

    #[test]
    fn unclosed_tag_is_an_error() {
        let err = parse("Hello <%= self.name ", &ParseOptions::default()).unwrap_err();
        assert!(err.message.contains("unclosed"));
        assert_eq!(err.span.start, 6);
    }

    #[test]
    fn whitespace_control_disabled_by_default() {
        // With trim off, `-%>` is not special; the `-` stays in the code.
        let nodes = parse_default("<% x -%>\ntail");
        match &nodes[0] {
            Node::Tag { code, .. } => assert_eq!(code, "x -"),
            other => panic!("expected tag, got {other:?}"),
        }
    }

    #[test]
    fn whitespace_control_trims_when_enabled() {
        let opts = ParseOptions { trim: true };
        let nodes = parse("<% x -%>\ntail", &opts).unwrap().nodes;
        // Newline after `-%>` is slurped; `x` is the code.
        match &nodes[0] {
            Node::Tag { code, .. } => assert_eq!(code, "x"),
            other => panic!("expected tag, got {other:?}"),
        }
        assert_eq!(
            nodes[1],
            Node::Text {
                span: nodes[1].span_for_test(),
                text: "tail".to_string(),
            }
        );
    }

    #[test]
    fn left_trim_slurps_preceding_whitespace() {
        let opts = ParseOptions { trim: true };
        let nodes = parse("line\n   <%_ x %>", &opts).unwrap().nodes;
        match &nodes[0] {
            Node::Text { text, .. } => assert_eq!(text, "line"),
            other => panic!("expected text, got {other:?}"),
        }
    }
}
