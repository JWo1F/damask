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

/// Info-string aliases, mapped to the name Syntect knows a syntax by.
///
/// `dmk` is the exception that justifies the table: it is a real syntax shipped
/// in `syntaxes/`, not an alias for HTML. Highlighting a Damask template as
/// plain HTML would leave every brace tag — the only part of the language that
/// is *not* HTML, and the reason the block is on the page — unstyled.
const ALIASES: &[(&str, &str)] = &[
    ("rs", "Rust"),
    ("rust", "Rust"),
    ("dmk", "Damask"),
    ("damask", "Damask"),
    ("html", "HTML"),
    ("css", "CSS"),
    ("toml", "TOML"),
    ("json", "JSON"),
    ("js", "JavaScript"),
    ("sh", "Bourne Again Shell (bash)"),
    ("bash", "Bourne Again Shell (bash)"),
    ("shell", "Bourne Again Shell (bash)"),
    ("console", "Bourne Again Shell (bash)"),
    ("yaml", "YAML"),
    ("md", "Markdown"),
    ("markdown", "Markdown"),
];

pub struct Highlighter {
    syntaxes: SyntaxSet,
    /// Info string → index into `syntaxes`, resolved once at startup so a page
    /// with forty code blocks does not do forty linear scans by name.
    by_alias: HashMap<&'static str, usize>,
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
            .filter_map(|(alias, name)| {
                let index = syntaxes.syntaxes().iter().position(|s| s.name == *name)?;
                Some((*alias, index))
            })
            .collect();

        Self { syntaxes, by_alias }
    }

    fn syntax(&self, language: &str) -> Option<&SyntaxReference> {
        let index = *self.by_alias.get(language.to_ascii_lowercase().as_str())?;
        self.syntaxes.syntaxes().get(index)
    }

    /// Renders one fenced block as a complete `<pre>`.
    ///
    /// The language travels onto the element as `data-lang` so the stylesheet
    /// can label the block without the template having to thread it through —
    /// the markdown pipeline hands back one opaque HTML string, and this is the
    /// last place that still knows what the code was.
    pub fn block(&self, language: &str, code: &str) -> String {
        let label = if language.is_empty() {
            String::new()
        } else {
            format!(" data-lang=\"{}\"", escape(language))
        };

        let body = match self.syntax(language) {
            Some(syntax) => self.tokenize(syntax, code),
            None => escape(code),
        };

        format!("<pre class=\"code\"{label}><code>{body}</code></pre>")
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
