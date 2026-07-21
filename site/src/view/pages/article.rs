use damask::Component;

use crate::content::{Kind, Page, Section};
use crate::markdown::Heading;
use crate::view::chrome::Chrome;
use crate::view::ui::{Step, Toc};

/// One document, in its collection.
///
/// The book and the reference share this page. They read differently, but the
/// difference is two booleans' worth — whether the sidebar numbers itself, and
/// whether there is a next chapter to offer — and two near-identical templates
/// would drift apart on everything else.
#[derive(Component)]
pub struct Article {
    pub chrome: Chrome,
    pub page: Page,
    pub sections: Vec<Section>,
    pub collection_title: String,
    pub collection_href: String,
    pub kind: Kind,
    pub previous: Option<Step>,
    pub next: Option<Step>,
}

impl Article {
    fn numbered(&self) -> bool {
        self.kind == Kind::Book
    }

    fn toc(&self) -> Option<Vec<Heading>> {
        Toc::entries(&self.page.headings)
    }

    /// A reference page gets no pager: its neighbours in the sidebar are a
    /// grouping, not a sequence, and "next" would invent a reading order the
    /// content does not claim.
    fn paged(&self) -> bool {
        self.kind == Kind::Book
    }
}
