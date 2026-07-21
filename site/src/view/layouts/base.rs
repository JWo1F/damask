use damask::Component;

use crate::view::chrome::Chrome;

/// The document.
#[derive(Component)]
pub struct Base {
    pub chrome: Chrome,
}

impl Base {
    /// Applies the stored theme before the first paint.
    ///
    /// Inline and blocking on purpose. A deferred or external script runs after
    /// the browser has already painted the default theme, which is a white flash
    /// on every navigation for a reader who chose the dark one — the one bug a
    /// dark theme cannot be shipped with.
    ///
    /// It is a Rust constant rather than markup because a `.dmk` reads `{` as a
    /// brace tag wherever it appears, script bodies included. Emitting the text
    /// through `{@html}` is what keeps the JavaScript out of the parser's way.
    const THEME_BOOT: &'static str = r#"<script>
(function () {
  try {
    var stored = localStorage.getItem("damask-theme");
    var dark = stored ? stored === "dark"
                      : matchMedia("(prefers-color-scheme: dark)").matches;
    document.documentElement.dataset.theme = dark ? "dark" : "light";
  } catch (e) {
    document.documentElement.dataset.theme = "light";
  }
})();
</script>"#;

    fn fonts() -> &'static str {
        "https://fonts.googleapis.com/css2\
         ?family=Fraunces:opsz,wght@9..144,400;9..144,500;9..144,600\
         &family=Newsreader:ital,opsz,wght@0,6..72,400;0,6..72,500;1,6..72,400\
         &family=Archivo:wght@400;500;600\
         &family=Spline+Sans+Mono:wght@400;500\
         &display=swap"
    }
}
