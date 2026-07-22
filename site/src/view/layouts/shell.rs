use damask::Component;

use crate::view::chrome::Chrome;

/// How far across the screen a page is allowed to run.
///
/// A measure that suits a chapter of prose is far too narrow for a three-panel
/// hero, and far too narrow again for a document page that has to hold two rails
/// of navigation beside its text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// A reading measure: the collection indexes, and anything that is prose all
    /// the way across.
    Text,
    /// The home page, whose sections are laid out rather than read.
    Wide,
    /// The whole viewport, for a page that puts its navigation in rails at the
    /// edges of the screen. The text between them keeps its own measure — see
    /// the column cap in `article.dmk`.
    Full,
}

/// Header, page, footer. Everything except the page's own content.
#[derive(Component)]
pub struct Shell {
    pub chrome: Chrome,
    pub width: Width,
}

impl Shell {
    fn width(&self) -> &'static str {
        match self.width {
            Width::Text => "max-w-[1080px]",
            Width::Wide => "max-w-[1180px]",
            Width::Full => "max-w-none",
        }
    }

    fn crates_io() -> &'static str {
        "https://crates.io/crates/damask"
    }

    fn docs_rs() -> &'static str {
        "https://docs.rs/damask"
    }
}
