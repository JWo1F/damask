use crate::{Node, Span, TagKind, Template};
use std::fmt;

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

/// Parse a `.rsc` template into a [`Template`].
///
/// Text is HTML; `{ … }` introduces a tag (see [`TagKind`]). Braces inside a tag
/// are balanced and string/char literals are respected, so struct literals and
/// closures work inside `{@render …}` and friends. `<!-- … -->` comments pass
/// through as literal text.
pub fn parse(src: &str) -> Result<Template, ParseError> {
    let bytes = src.as_bytes();
    let n = bytes.len();

    let mut nodes: Vec<Node> = Vec::new();
    let mut text = String::new();
    let mut text_start = 0usize;
    let mut i = 0usize;

    while i < n {
        // HTML comments pass through verbatim — don't parse `{ }` inside them.
        if bytes[i] == b'<' && src[i..].starts_with("<!--") {
            if text.is_empty() {
                text_start = i;
            }
            let end = match src[i + 4..].find("-->") {
                Some(rel) => i + 4 + rel + 3,
                None => n,
            };
            text.push_str(&src[i..end]);
            i = end;
            continue;
        }

        if bytes[i] == b'{' {
            if !text.is_empty() {
                nodes.push(Node::Text {
                    span: Span::new(text_start, i),
                    text: std::mem::take(&mut text),
                });
            }
            let tag = scan_tag(src, i)?;
            nodes.push(Node::Tag {
                span: Span::new(i, tag.end),
                kind: tag.kind,
                code: tag.code,
                code_span: tag.code_span,
            });
            i = tag.end;
            text_start = i;
            continue;
        }

        if text.is_empty() {
            text_start = i;
        }
        let ch = src[i..].chars().next().expect("char boundary");
        text.push(ch);
        i += ch.len_utf8();
    }

    if !text.is_empty() {
        nodes.push(Node::Text {
            span: Span::new(text_start, n),
            text,
        });
    }

    Ok(Template { nodes })
}

/// Whether `offset` lies inside an open `{ … }` tag (brace depth > 0), tolerant
/// of half-typed tags. Used by the language server for completion context.
///
/// Braces inside strings/char literals (within a tag) don't count, and a cursor
/// inside an HTML comment is not in a tag.
pub fn in_tag(src: &str, offset: usize) -> bool {
    let offset = offset.min(src.len());
    let bytes = src.as_bytes();
    let mut i = 0usize;
    let mut depth: i32 = 0;

    while i < offset {
        if bytes[i] == b'<' && src[i..].starts_with("<!--") {
            let end = match src[i + 4..].find("-->") {
                Some(rel) => i + 4 + rel + 3,
                None => src.len(),
            };
            if end > offset {
                return false; // cursor is inside a comment
            }
            i = end;
            continue;
        }
        match bytes[i] {
            b'"' if depth > 0 => i = scan_string(src, i),
            b'\'' if depth > 0 => i = scan_char(src, i),
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = (depth - 1).max(0);
                i += 1;
            }
            _ => {
                let ch = src[i..].chars().next().expect("char boundary");
                i += ch.len_utf8();
            }
        }
    }

    depth > 0
}

struct ScannedTag {
    kind: TagKind,
    code: String,
    code_span: Span,
    end: usize,
}

/// Scan one `{ … }` tag beginning at `open` (the `{`), balancing nested braces
/// and skipping string/char literals.
fn scan_tag(src: &str, open: usize) -> Result<ScannedTag, ParseError> {
    let bytes = src.as_bytes();
    let n = bytes.len();

    let inner_start = open + 1;
    let mut i = inner_start;
    let mut depth = 1usize;

    while i < n {
        match bytes[i] {
            b'"' => i = scan_string(src, i),
            b'\'' => i = scan_char(src, i),
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let inner = &src[inner_start..i];
                    let lead = inner.len() - inner.trim_start().len();
                    let trimmed = inner.trim();
                    let code_start = inner_start + lead;
                    let code_span = Span::new(code_start, code_start + trimmed.len());
                    let (kind, code) = classify(trimmed).map_err(|message| ParseError {
                        message,
                        span: Span::new(open, i + 1),
                    })?;
                    return Ok(ScannedTag {
                        kind,
                        code,
                        code_span,
                        end: i + 1,
                    });
                }
                i += 1;
            }
            _ => {
                let ch = src[i..].chars().next().expect("char boundary");
                i += ch.len_utf8();
            }
        }
    }

    Err(ParseError {
        message: "unclosed tag: missing `}`".to_string(),
        span: Span::new(open, n),
    })
}

/// Classify the trimmed inner content of a tag into a [`TagKind`] and the
/// meaningful code (sigil and block keyword stripped).
fn classify(t: &str) -> Result<(TagKind, String), String> {
    if let Some(body) = t.strip_prefix('@') {
        let body = body.trim_start();
        if let Some(rest) = keyword(body, "html") {
            return Ok((TagKind::Html, rest));
        }
        if let Some(rest) = keyword(body, "const") {
            return Ok((TagKind::Const, rest));
        }
        if let Some(rest) = keyword(body, "render") {
            return Ok((TagKind::Render, rest));
        }
        return Err(format!("unknown directive `{{@{body}}}`; expected @html, @const, or @render"));
    }

    if let Some(body) = t.strip_prefix('#') {
        let body = body.trim_start();
        if let Some(rest) = keyword(body, "if") {
            return Ok((TagKind::If, rest));
        }
        if let Some(rest) = keyword(body, "each") {
            return Ok((TagKind::Each, rest));
        }
        if let Some(rest) = keyword(body, "snippet") {
            return Ok((TagKind::Snippet, rest));
        }
        return Err(format!("unknown block `{{#{body}}}`; expected #if, #each, or #snippet"));
    }

    if let Some(body) = t.strip_prefix(':') {
        let body = body.trim_start();
        if let Some(rest) = keyword(body, "else") {
            let rest = rest.trim_start();
            if let Some(cond) = keyword(rest, "if") {
                return Ok((TagKind::ElseIf, cond));
            }
            if rest.is_empty() {
                return Ok((TagKind::Else, String::new()));
            }
            return Err(format!("unexpected `{rest}` after `:else`"));
        }
        return Err(format!("unknown clause `{{:{body}}}`; expected :else or :else if"));
    }

    if let Some(body) = t.strip_prefix('/') {
        let body = body.trim();
        if matches!(body, "if" | "each" | "snippet") {
            return Ok((TagKind::Close, body.to_string()));
        }
        return Err(format!("unknown closing tag `{{/{body}}}`"));
    }

    Ok((TagKind::Expr, t.to_string()))
}

/// If `s` begins with keyword `k` at a word boundary, return the trimmed
/// remainder.
fn keyword(s: &str, k: &str) -> Option<String> {
    let rest = s.strip_prefix(k)?;
    match rest.chars().next() {
        None => Some(String::new()),
        Some(c) if c.is_alphanumeric() || c == '_' => None,
        Some(_) => Some(rest.trim().to_string()),
    }
}

/// `i` points at an opening `"`. Return the index just past the closing quote
/// (or end of input for an unterminated string).
fn scan_string(src: &str, i: usize) -> usize {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut j = i + 1;
    while j < n {
        match bytes[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => {
                let ch = src[j..].chars().next().expect("char boundary");
                j += ch.len_utf8();
            }
        }
    }
    n
}

/// `i` points at a `'`. If it opens a char literal (`'x'`, `'\n'`, `'}'`), skip
/// past it; otherwise it is a lifetime, so skip only the quote.
fn scan_char(src: &str, i: usize) -> usize {
    let bytes = src.as_bytes();
    let n = bytes.len();
    let mut j = i + 1;

    if j < n && bytes[j] == b'\\' {
        j += 2; // escape sequence
        if j < n && bytes[j] == b'\'' {
            return j + 1;
        }
    } else if j < n {
        let ch = src[j..].chars().next().expect("char boundary");
        let after = j + ch.len_utf8();
        if after < n && bytes[after] == b'\'' {
            return after + 1;
        }
    }

    i + 1 // a lifetime, not a char literal
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(src: &str) -> Vec<(TagKind, String)> {
        parse(src)
            .unwrap()
            .nodes
            .into_iter()
            .filter_map(|n| match n {
                Node::Tag { kind, code, .. } => Some((kind, code)),
                _ => None,
            })
            .collect()
    }

    fn texts(src: &str) -> Vec<String> {
        parse(src)
            .unwrap()
            .nodes
            .into_iter()
            .filter_map(|n| match n {
                Node::Text { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn plain_text() {
        assert_eq!(texts("Hello, world!"), vec!["Hello, world!"]);
        assert_eq!(tags("Hello, world!"), vec![]);
    }

    #[test]
    fn expression_tag() {
        assert_eq!(
            tags("Hello {self.name}!"),
            vec![(TagKind::Expr, "self.name".to_string())]
        );
    }

    #[test]
    fn directives() {
        assert_eq!(tags("{@html self.body}"), vec![(TagKind::Html, "self.body".into())]);
        assert_eq!(tags("{@const x = 1}"), vec![(TagKind::Const, "x = 1".into())]);
        assert_eq!(tags("{@render self.footer}"), vec![(TagKind::Render, "self.footer".into())]);
    }

    #[test]
    fn if_block() {
        assert_eq!(
            tags("{#if self.admin}A{:else if self.guest}G{:else}U{/if}"),
            vec![
                (TagKind::If, "self.admin".into()),
                (TagKind::ElseIf, "self.guest".into()),
                (TagKind::Else, String::new()),
                (TagKind::Close, "if".into()),
            ]
        );
    }

    #[test]
    fn each_block_with_and_without_index() {
        assert_eq!(
            tags("{#each &self.items as item}x{/each}"),
            vec![
                (TagKind::Each, "&self.items as item".into()),
                (TagKind::Close, "each".into())
            ]
        );
        assert_eq!(
            tags("{#each &self.items as item, i}x{/each}")[0],
            (TagKind::Each, "&self.items as item, i".into())
        );
    }

    #[test]
    fn snippet_block() {
        assert_eq!(
            tags("{#snippet row(item)}x{/snippet}"),
            vec![
                (TagKind::Snippet, "row(item)".into()),
                (TagKind::Close, "snippet".into())
            ]
        );
    }

    #[test]
    fn render_with_nested_braces() {
        // The struct literal's braces must not close the tag early.
        assert_eq!(
            tags(r#"{@render Card { title: "hi".into() }}"#),
            vec![(TagKind::Render, r#"Card { title: "hi".into() }"#.into())]
        );
    }

    #[test]
    fn string_with_brace_does_not_close_tag() {
        assert_eq!(tags(r#"{ "}" }"#), vec![(TagKind::Expr, r#""}""#.into())]);
    }

    #[test]
    fn char_literal_brace() {
        assert_eq!(tags("{ f('}') }"), vec![(TagKind::Expr, "f('}')".into())]);
    }

    #[test]
    fn html_comment_passes_through() {
        assert_eq!(texts("a<!-- {not a tag} -->b"), vec!["a<!-- {not a tag} -->b"]);
        assert_eq!(tags("a<!-- {not a tag} -->b"), vec![]);
    }

    #[test]
    fn unclosed_tag_errors() {
        let err = parse("Hello {self.name").unwrap_err();
        assert!(err.message.contains("unclosed"));
        assert_eq!(err.span.start, 6);
    }

    #[test]
    fn unknown_block_errors() {
        assert!(parse("{#wat}x{/wat}").unwrap_err().message.contains("unknown block"));
    }

    #[test]
    fn multibyte_text() {
        assert_eq!(texts("héllo {x} 😀"), vec!["héllo ", " 😀"]);
    }
}
