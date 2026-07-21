use damask::Component;

use crate::content::Section;

/// The collection's own navigation: every page in it, grouped.
///
/// The same component serves the book and the reference. A book has one unnamed
/// section, so the group heading simply does not render — the difference between
/// "a numbered list of chapters" and "a grouped index" is entirely in the
/// content, which is where it belongs.
#[derive(Component)]
pub struct Sidebar {
    pub sections: Vec<Section>,
    /// The slug of the page being read, so it can mark itself.
    pub current: String,
    /// Numbers the entries, for a collection meant to be read in order.
    pub numbered: bool,
}
