use damask::Component;

/// One entry in a collection's index.
///
/// The number is passed in rather than derived, because the index lists pages
/// across sections and the count has to run through all of them.
#[derive(Component)]
pub struct PageCard {
    pub href: String,
    pub title: String,
    /// Rendered inline markdown, emitted with `{@html}`.
    pub summary: String,
    pub position: Option<usize>,
}
