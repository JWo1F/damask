use damask::Component;

use crate::content::Page;

/// Where to go from here.
#[derive(Debug, Clone)]
pub struct Step {
    pub title: String,
    pub href: String,
}

impl Step {
    pub fn of(page: Option<&Page>) -> Option<Self> {
        page.map(|page| Self {
            title: page.title.clone(),
            href: page.href.clone(),
        })
    }
}

/// Previous / next, at the foot of a page.
///
/// Both sides are optional and the first and last pages each have one, so the
/// two are placed by their own alignment rather than by a spacer — a `justify-
/// between` with one child would pull a lone "Next" to the left, where it reads
/// as "Previous".
#[derive(Component)]
pub struct Pager {
    pub previous: Option<Step>,
    pub next: Option<Step>,
}
