use damask::Component;

use crate::content::{Collection, Kind};
use crate::view::chrome::Chrome;

/// A collection's landing page: its prose, then every page in it.
#[derive(Component)]
pub struct Index {
    pub chrome: Chrome,
    pub collection: Collection,
}

impl Index {
    /// The book numbers its chapters; the reference does not.
    ///
    /// Reading order is a fact about the book — a reader who opens chapter four
    /// first should be told there were three before it. Reference pages are
    /// entered by lookup, and numbering them would imply an order that is not
    /// there.
    fn numbered(&self) -> bool {
        self.collection.kind == Kind::Book
    }
}
