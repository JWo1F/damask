/// The host language of a template, derived from the middle extension of its
/// filename (`show.html.rsc` → [`Html`](HostLang::Html)).
///
/// The host language selects the default renderer for a component (see the
/// `rsc` crate) and the outer injection language for editor tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLang {
    Html,
    Js,
    Css,
    /// No recognized middle extension (e.g. `notes.rsc`); no escaping.
    Plain,
}

impl HostLang {
    /// Determine the host language from a template file name.
    ///
    /// The rule: strip a trailing `.rsc`, then look at the extension that
    /// remains. `button.html.rsc` → `Html`, `app.js.rsc` → `Js`,
    /// `theme.css.rsc` → `Css`, `notes.rsc` (or an unknown middle extension) →
    /// `Plain`.
    pub fn from_filename(name: &str) -> HostLang {
        // Use only the final path component, in case a full path was passed.
        let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let stem = file.strip_suffix(".rsc").unwrap_or(file);
        match stem.rsplit_once('.') {
            Some((_, ext)) => Self::from_extension(ext),
            None => HostLang::Plain,
        }
    }

    /// Map a bare extension (no dot) to a host language.
    pub fn from_extension(ext: &str) -> HostLang {
        match ext.to_ascii_lowercase().as_str() {
            "html" | "htm" | "xhtml" => HostLang::Html,
            "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" => HostLang::Js,
            "css" | "scss" | "sass" => HostLang::Css,
            _ => HostLang::Plain,
        }
    }

    /// The unqualified name of the built-in renderer for this language, as
    /// exposed by `rsc::renderers`. Used by the `component!` macro to pick a
    /// component's default renderer.
    pub fn renderer_type(self) -> &'static str {
        match self {
            HostLang::Html => "HtmlRenderer",
            HostLang::Js => "JsRenderer",
            HostLang::Css => "CssRenderer",
            HostLang::Plain => "PlainRenderer",
        }
    }

    /// The Tree-sitter / LSP language name to inject into the literal text
    /// regions of a template of this host language.
    pub fn injection_language(self) -> &'static str {
        match self {
            HostLang::Html => "html",
            HostLang::Js => "javascript",
            HostLang::Css => "css",
            HostLang::Plain => "text",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HostLang;

    #[test]
    fn from_filename_maps_middle_extension() {
        assert_eq!(HostLang::from_filename("show.html.rsc"), HostLang::Html);
        assert_eq!(HostLang::from_filename("banner.js.rsc"), HostLang::Js);
        assert_eq!(HostLang::from_filename("styles.css.rsc"), HostLang::Css);
        assert_eq!(HostLang::from_filename("notes.rsc"), HostLang::Plain);
        assert_eq!(HostLang::from_filename("weird.txt.rsc"), HostLang::Plain);
    }

    #[test]
    fn from_filename_handles_paths_and_case() {
        assert_eq!(
            HostLang::from_filename("/a/b/Button.HTML.rsc"),
            HostLang::Html
        );
        assert_eq!(
            HostLang::from_filename(r"src\components\card.js.rsc"),
            HostLang::Js
        );
    }

    #[test]
    fn renderer_type_matches_language() {
        assert_eq!(HostLang::Html.renderer_type(), "HtmlRenderer");
        assert_eq!(HostLang::Plain.renderer_type(), "PlainRenderer");
    }
}
