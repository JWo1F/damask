use damask::Component;

use crate::content;
use crate::view::chrome::Chrome;

/// A feature block, with its code already highlighted.
///
/// The content model carries the code as source; highlighting happens once, in
/// the generator, so the template is handed markup and this struct is what
/// carries it across that boundary.
#[derive(Debug, Clone)]
pub struct Feature {
    pub title: String,
    /// The body as rendered inline markdown, emitted with `{@html}`.
    pub body_html: String,
    pub code: Option<String>,
}

#[derive(Component)]
pub struct Home {
    pub chrome: Chrome,
    pub content: content::Home,
    pub features: Vec<Feature>,
    /// The dependency snippet, highlighted.
    pub install: String,
    /// The hero's three panels, highlighted.
    pub rs: String,
    pub dmk: String,
    pub out: String,
    /// Where "read the book" goes, for the closing invitation.
    pub book_href: String,
    pub docs_href: String,
}
