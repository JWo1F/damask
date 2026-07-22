//! Build-time syntax highlighting for fenced code blocks.
//!
//! Syntect is driven in its *classed* mode rather than its themed one: it emits
//! `<span class="tok-keyword …">` and the colours live in `ui/app.css`. Inline
//! styles would hard-code one theme into the markup, and this site has two — a
//! reader on a dark screen would get a light theme's syntax colours baked into
//! the HTML, with no stylesheet able to reach them.

use std::collections::HashMap;

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Prefixed so the token classes cannot collide with a Tailwind utility.
const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "tok-" };

/// Info-string aliases: what an author writes, the name Syntect knows the
/// syntax by, and what the block is labelled with on the page.
///
/// `dmk` is the exception that justifies the table: it is a real syntax shipped
/// in `syntaxes/`, not an alias for HTML. Highlighting a Damask template as
/// plain HTML would leave every brace tag — the only part of the language that
/// is *not* HTML, and the reason the block is on the page — unstyled.
///
/// The third column exists because the middle one is Syntect's vocabulary, not
/// a reader's: `sh` would otherwise be labelled "Bourne Again Shell (bash)".
const ALIASES: &[(&str, &str, &str)] = &[
    ("rs", "Rust", "Rust"),
    ("rust", "Rust", "Rust"),
    ("dmk", "Damask", "Damask"),
    ("damask", "Damask", "Damask"),
    ("html", "HTML", "HTML"),
    ("css", "CSS", "CSS"),
    ("toml", "TOML", "TOML"),
    ("json", "JSON", "JSON"),
    ("js", "JavaScript", "JavaScript"),
    ("sh", "Bourne Again Shell (bash)", "Shell"),
    ("bash", "Bourne Again Shell (bash)", "Bash"),
    ("shell", "Bourne Again Shell (bash)", "Shell"),
    ("console", "Bourne Again Shell (bash)", "Console"),
    ("yaml", "YAML", "YAML"),
    ("md", "Markdown", "Markdown"),
    ("markdown", "Markdown", "Markdown"),
];

pub struct Highlighter {
    syntaxes: SyntaxSet,
    /// Info string → index into `syntaxes`, resolved once at startup so a page
    /// with forty code blocks does not do forty linear scans by name.
    by_alias: HashMap<&'static str, usize>,
    /// Info string → the label the rail shows.
    labels: HashMap<&'static str, &'static str>,
}

impl Highlighter {
    /// Loads the bundled syntaxes plus the Damask one in `syntaxes/`.
    pub fn new() -> Self {
        let mut builder = SyntaxSet::load_defaults_newlines().into_builder();

        // Bundled with the generator rather than read from disk: the binary is
        // run from wherever, and a highlighter that silently degrades to plain
        // text because the working directory moved is the kind of failure that
        // ships.
        let damask = syntect::parsing::SyntaxDefinition::load_from_str(
            include_str!("../syntaxes/damask.sublime-syntax"),
            true,
            None,
        )
        .expect("parse the bundled Damask syntax definition");
        builder.add(damask);

        let syntaxes = builder.build();
        let by_alias = ALIASES
            .iter()
            .filter_map(|(alias, name, _)| {
                let index = syntaxes.syntaxes().iter().position(|s| s.name == *name)?;
                Some((*alias, index))
            })
            .collect();
        let labels = ALIASES
            .iter()
            .map(|(alias, _, label)| (*alias, *label))
            .collect();

        Self {
            syntaxes,
            by_alias,
            labels,
        }
    }

    fn syntax(&self, language: &str) -> Option<&SyntaxReference> {
        let index = *self.by_alias.get(language.to_ascii_lowercase().as_str())?;
        self.syntaxes.syntaxes().get(index)
    }

    /// Renders one fenced block as a bare `<pre>`, for callers that draw their
    /// own chrome around it — the home page's filename panels already have a
    /// caption, and a second one under it would label the same code twice.
    pub fn pre(&self, language: &str, code: &str) -> String {
        // Comrak hands over the fence body with its final newline attached.
        // Kept, it renders as a blank line the author did not write — visible
        // as a gap between the last line of code and the block's bottom edge.
        let code = code.strip_suffix('\n').unwrap_or(code);

        let body = match self.syntax(language) {
            Some(syntax) => self.tokenize(syntax, code),
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

    fn tokenize(&self, syntax: &SyntaxReference, code: &str) -> String {
        let mut generator =
            ClassedHTMLGenerator::new_with_class_style(syntax, &self.syntaxes, CLASS_STYLE);

        for line in LinesWithEndings::from(code) {
            // A malformed line is not worth failing a build over, and Syntect
            // leaves the generator usable: the block simply loses colour from
            // here on rather than taking the site down with it.
            if generator
                .parse_html_for_line_which_includes_newline(line)
                .is_err()
            {
                return escape(code);
            }
        }

        generator.finalize()
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
