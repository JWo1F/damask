//! Build-time syntax highlighting for fenced code blocks.
//!
//! Tree-sitter parses each block and the classed spans it produces are coloured
//! by `ui/app.css`. Inline styles would hard-code one theme into the markup, and
//! this site has two — a reader on a dark screen would get a light theme's
//! syntax colours baked into the HTML, with no stylesheet able to reach them.
//!
//! Damask is highlighted by the same grammar and the same queries the editors
//! use: `crates/tree-sitter-damask` vendors the parser, and the queries are read
//! straight out of the Zed extension. A `.dmk` snippet therefore looks the same
//! on this site as it does in an editor, and there is one place to fix it when
//! it looks wrong.

use std::collections::HashMap;

use tree_sitter_highlight::{Highlight, HighlightConfiguration, HtmlRenderer};

/// Every capture name the bundled queries use, paired with the class it becomes.
///
/// This is the whole vocabulary: a capture the table does not name is left
/// unhighlighted, and `configure` below is what enforces that. Several names
/// share a class deliberately — the site's palette has nine syntax colours, and
/// splitting them further would mean inventing distinctions a reader cannot see.
///
/// Tree-sitter resolves a capture against the longest matching prefix, so
/// `@comment.documentation` finds `comment` without being listed.
const CLASSES: &[(&str, &str)] = &[
    ("attribute", "tok-attr"),
    ("boolean", "tok-number"),
    // CSS at-rules. Each is its own capture upstream, and each is a keyword.
    ("charset", "tok-keyword"),
    ("comment", "tok-comment"),
    ("constant", "tok-number"),
    // A capitalised path segment or an enum variant: a name for a type.
    ("constructor", "tok-type"),
    // The `$( … )` and `${ … }` of a shell substitution. Only the wrapper takes
    // this class — what is captured inside it nests, and wins.
    ("embedded", "tok-brace"),
    ("escape", "tok-number"),
    ("function", "tok-function"),
    ("import", "tok-keyword"),
    ("keyframes", "tok-keyword"),
    ("keyword", "tok-keyword"),
    // A Rust lifetime.
    ("label", "tok-keyword"),
    ("media", "tok-keyword"),
    ("namespace", "tok-keyword"),
    ("number", "tok-number"),
    ("operator", "tok-punct"),
    // A field, a shell variable, a TOML key, a CSS property — a member name.
    ("property", "tok-attr"),
    ("punctuation.bracket", "tok-punct"),
    ("punctuation.delimiter", "tok-punct"),
    // A Damask `{`, `}` or `{#`: the one part of a template that is not HTML,
    // and the reason the block is on the page. It takes the accent colour.
    ("punctuation.special", "tok-brace"),
    ("string", "tok-string"),
    // A class name in a Damask `class` list or map. It is not Rust and not a
    // plain string, but it reads as the literal it is.
    ("string.special", "tok-string"),
    ("supports", "tok-keyword"),
    ("tag", "tok-tag"),
    ("type", "tok-type"),
    ("variable", "tok-variable"),
    // `self`.
    ("variable.builtin", "tok-builtin"),
    ("variable.parameter", "tok-variable"),
];

/// Info-string aliases: what an author writes, the grammar it selects, and what
/// the block is labelled with on the page.
///
/// The grammar column doubles as the name an injection asks for — a Damask tag
/// asks for `rust`, a `<style>` asks for `css` — so one table answers both.
///
/// The third column exists because a reader is not owed the grammar's name:
/// `sh` and `bash` are one grammar and two labels.
const ALIASES: &[(&str, &str, &str)] = &[
    ("rs", "rust", "Rust"),
    ("rust", "rust", "Rust"),
    ("dmk", "damask", "Damask"),
    ("damask", "damask", "Damask"),
    ("html", "html", "HTML"),
    ("css", "css", "CSS"),
    ("toml", "toml", "TOML"),
    ("sh", "bash", "Shell"),
    ("bash", "bash", "Bash"),
    ("shell", "bash", "Shell"),
    ("console", "bash", "Console"),
];

/// The grammars, their queries, and the name an injection knows them by.
///
/// Damask's queries are the Zed extension's, read from the source tree rather
/// than copied here: two copies of a highlight query drift, and the drift shows
/// up as a snippet that is coloured one way in an editor and another way on the
/// page documenting it.
fn grammars() -> Vec<(&'static str, HighlightConfiguration)> {
    let sources: [(&str, tree_sitter::Language, &str, &str); 6] = [
        (
            "damask",
            tree_sitter_damask::LANGUAGE.into(),
            include_str!("../../editors/zed/languages/damask/highlights.scm"),
            include_str!("../../editors/zed/languages/damask/injections.scm"),
        ),
        (
            "rust",
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
        ),
        (
            "html",
            tree_sitter_html::LANGUAGE.into(),
            tree_sitter_html::HIGHLIGHTS_QUERY,
            tree_sitter_html::INJECTIONS_QUERY,
        ),
        (
            "css",
            tree_sitter_css::LANGUAGE.into(),
            tree_sitter_css::HIGHLIGHTS_QUERY,
            "",
        ),
        (
            "toml",
            tree_sitter_toml_ng::LANGUAGE.into(),
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
        ),
        (
            "bash",
            tree_sitter_bash::LANGUAGE.into(),
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
        ),
    ];

    let names: Vec<&str> = CLASSES.iter().map(|(name, _)| *name).collect();

    sources
        .into_iter()
        .map(|(name, language, highlights, injections)| {
            let mut config =
                HighlightConfiguration::new(language, name, highlights, injections, "")
                    .unwrap_or_else(|error| {
                        panic!("compile the {name} highlight queries: {error}")
                    });

            // Binds each capture to its index in `CLASSES`; anything the table
            // does not name stops producing a span at all.
            config.configure(&names);
            (name, config)
        })
        .collect()
}

pub struct Highlighter {
    grammars: Vec<(&'static str, HighlightConfiguration)>,
    /// Info string → index into `grammars`, resolved once at startup so a page
    /// with forty code blocks does not do forty linear scans by name.
    by_alias: HashMap<&'static str, usize>,
    /// Info string → the label the rail shows.
    labels: HashMap<&'static str, &'static str>,
}

impl Highlighter {
    pub fn new() -> Self {
        let grammars = grammars();
        let by_alias = ALIASES
            .iter()
            .filter_map(|(alias, grammar, _)| {
                let index = grammars.iter().position(|(name, _)| name == grammar)?;
                Some((*alias, index))
            })
            .collect();
        let labels = ALIASES
            .iter()
            .map(|(alias, _, label)| (*alias, *label))
            .collect();

        Self {
            grammars,
            by_alias,
            labels,
        }
    }

    fn config(&self, language: &str) -> Option<&HighlightConfiguration> {
        let index = *self.by_alias.get(language.to_ascii_lowercase().as_str())?;
        Some(&self.grammars[index].1)
    }

    /// Renders one fenced block as a bare `<pre>`, for callers that draw their
    /// own chrome around it — the home page's filename panels already have a
    /// caption, and a second one under it would label the same code twice.
    pub fn pre(&self, language: &str, code: &str) -> String {
        // Comrak hands over the fence body with its final newline attached.
        // Kept, it renders as a blank line the author did not write — visible
        // as a gap between the last line of code and the block's bottom edge.
        let code = code.strip_suffix('\n').unwrap_or(code);

        let body = match self.config(language) {
            Some(config) => self.tokenize(config, code).unwrap_or_else(|| escape(code)),
            None => escape(code),
        };

        format!("<pre class=\"code\"><code>{body}</code></pre>")
    }

    /// Renders one fenced block as a complete, framed component: a rail
    /// carrying the language and the copy button, over a scroll region.
    ///
    /// The frame is built here rather than in a template because the markdown
    /// pipeline hands back one opaque HTML string — this is the last place that
    /// still knows what the code was. Building it here rather than in `site.js`
    /// is what keeps the block whole with scripting off, and keeps the page
    /// from reflowing around chrome that appears after the first paint.
    ///
    /// The button itself is *not* emitted: it is the one part that does nothing
    /// without JavaScript, so `site.js` adds it and the rail simply carries the
    /// label alone when it cannot.
    pub fn block(&self, language: &str, code: &str) -> String {
        let (attribute, label) = if language.is_empty() {
            (String::new(), String::new())
        } else {
            let label = self
                .labels
                .get(language.to_ascii_lowercase().as_str())
                .map(|label| (*label).to_string())
                .unwrap_or_else(|| language.to_string());
            (
                format!(" data-lang=\"{}\"", escape(language)),
                escape(&label),
            )
        };

        format!(
            "<figure class=\"code-block\"{attribute}>\
             <figcaption class=\"code-rail\" data-code-rail>\
             <span class=\"code-lang\">{label}</span>\
             </figcaption>\
             <div class=\"code-scroll\">{}</div>\
             </figure>",
            self.pre(language, code)
        )
    }

    /// `None` when the block cannot be highlighted, which the caller renders as
    /// escaped text. A snippet the grammar chokes on is not worth failing a
    /// build over: it loses its colour and keeps its content.
    fn tokenize(&self, config: &HighlightConfiguration, code: &str) -> Option<String> {
        // Built per block rather than held on `self`: the engine needs `&mut`
        // to run, `Highlighter` is shared as `&` by every caller, and a build
        // that highlights a couple of hundred blocks does not notice the
        // allocation. Interior mutability would buy nothing but a `RefCell`.
        let mut engine = tree_sitter_highlight::Highlighter::new();

        let source = code.as_bytes();
        let events = engine
            // A grammar may inject a language the site does not carry —
            // `<script>` asks for JavaScript. The lookup returns `None` and
            // that region stays plain text inside an otherwise coloured block,
            // which is what an unknown fence already does.
            .highlight(config, source, None, |name| self.config(name))
            .ok()?;

        let mut renderer = HtmlRenderer::new();
        renderer
            .render(events, source, &|Highlight(index), out| {
                out.extend_from_slice(b"class=\"");
                out.extend_from_slice(CLASSES[index].1.as_bytes());
                out.push(b'"');
            })
            .ok()?;

        // The renderer terminates its output with a newline whether or not the
        // source had one. Dropped for the same reason the source's is above.
        let html: String = renderer.lines().collect();
        Some(html.strip_suffix('\n').unwrap_or(&html).to_string())
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(character),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::Highlighter;

    /// The classes a block is made of, in order, so a test can assert about
    /// what was highlighted without pinning the exact markup around it.
    ///
    /// Spans nest — a grammar may capture a node and its parent — so `tokens`
    /// below reads leaves. Assert against a token nothing else encloses.
    fn classes(html: &str) -> Vec<&str> {
        html.match_indices("class=\"tok-")
            .map(|(at, _)| {
                let rest = &html[at + "class=\"".len()..];
                &rest[..rest.find('"').expect("a closed class attribute")]
            })
            .collect()
    }

    fn tokens<'a>(html: &'a str, class: &str) -> Vec<&'a str> {
        let open = format!("<span class=\"{class}\">");
        html.match_indices(&open)
            .map(|(at, _)| {
                let rest = &html[at + open.len()..];
                &rest[..rest.find("</span>").expect("a closed span")]
            })
            .collect()
    }

    #[test]
    fn a_template_is_markup_with_rust_in_it() {
        let html = Highlighter::new().pre("dmk", "<p class=\"lead\">{ self.name }</p>");

        assert_eq!(tokens(&html, "tok-tag"), ["p", "p"]);
        assert_eq!(tokens(&html, "tok-attr"), ["class", "name"]);
        // The braces carry the accent: they are the part that is not HTML.
        assert_eq!(tokens(&html, "tok-brace"), ["{", "}"]);
        // `self` is Rust, which means the injection fired.
        assert_eq!(tokens(&html, "tok-builtin"), ["self"]);
    }

    /// Every `{ … }` region injects Rust, and injects it *whole*. Both halves
    /// have been wrong: a string literal inside a tag was cut out of the
    /// injected range, and a class value injected an empty range and so got no
    /// colour at all. See `editors/zed/languages/damask/injections.scm`.
    #[test]
    fn every_expression_injects_whole() {
        let highlighter = Highlighter::new();

        for template in [
            r#"<p>{ "hi".len() }</p>"#,
            r#"<p>{#if "hi".len() > self.n}x{/if}</p>"#,
            r#"<div class={ "on": "hi".len() > self.n }>"#,
            r#"<div class=["a", "hi".len()]>"#,
            r#"<div class:on={ "hi".len() > self.n }>"#,
            r#"<Card {..."hi".len()}/>"#,
        ] {
            let html = highlighter.pre("dmk", template);
            assert!(
                tokens(&html, "tok-function").contains(&"len"),
                "no Rust injected into {template}: {html}"
            );
            assert!(
                tokens(&html, "tok-string").contains(&"&quot;hi&quot;"),
                "the literal fell outside the injection in {template}: {html}"
            );
        }
    }

    #[test]
    fn the_other_grammars_load() {
        let highlighter = Highlighter::new();

        for (language, code, class, token) in [
            ("rust", "let n = 1;", "tok-keyword", "let"),
            ("html", "<b>hi</b>", "tok-tag", "b"),
            ("css", ".a { color: red; }", "tok-attr", "color"),
            ("toml", "n = 1", "tok-number", "1"),
            ("sh", "# note", "tok-comment", "# note"),
        ] {
            let html = highlighter.pre(language, code);
            assert!(
                tokens(&html, class).contains(&token),
                "{language} did not highlight {token:?}: {html}"
            );
        }
    }

    /// A fence the site has no grammar for keeps its content and loses only its
    /// colour, which is also what a block the parser chokes on falls back to.
    #[test]
    fn an_unknown_fence_is_escaped_plain_text() {
        let html = Highlighter::new().pre("text", "a <b> & \"c\"");

        assert!(classes(&html).is_empty(), "{html}");
        assert!(html.contains("a &lt;b&gt; &amp; &quot;c&quot;"), "{html}");
    }

    /// The rail labels a block for a reader, not for the parser: `sh` and
    /// `bash` are one grammar and two labels, and an unknown fence is labelled
    /// with whatever the author wrote.
    #[test]
    fn the_rail_carries_the_label() {
        let highlighter = Highlighter::new();

        for (language, label) in [("sh", "Shell"), ("bash", "Bash"), ("nix", "nix")] {
            let html = highlighter.block(language, "x");
            assert!(
                html.contains(&format!("<span class=\"code-lang\">{label}</span>")),
                "{language} was not labelled {label}: {html}"
            );
        }
    }

    /// The renderer terminates its output with a newline of its own; kept, it
    /// would show as a blank line under the last line of every block.
    #[test]
    fn a_block_does_not_end_in_a_blank_line() {
        let html = Highlighter::new().pre("rust", "let n = 1;\n");

        assert!(html.ends_with("</code></pre>"), "{html}");
        assert!(!html.contains("\n</code>"), "{html}");
    }
}
