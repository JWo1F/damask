use damask::Component;

use crate::view::chrome::Chrome;

/// Header, page, footer. Everything except the page's own content.
#[derive(Component)]
pub struct Shell {
    pub chrome: Chrome,
    /// Widens the content column for the home page, whose sections are laid out
    /// rather than read. A measure that suits a chapter of prose is far too
    /// narrow for a three-panel hero.
    pub wide: bool,
}

impl Shell {
    fn width(&self) -> &'static str {
        if self.wide {
            "max-w-[1180px]"
        } else {
            "max-w-[1080px]"
        }
    }

    fn crates_io() -> &'static str {
        "https://crates.io/crates/damask"
    }

    fn docs_rs() -> &'static str {
        "https://docs.rs/damask"
    }
}
