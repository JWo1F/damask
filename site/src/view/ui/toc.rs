use damask::Component;

use crate::markdown::Heading;

/// On-page contents.
///
/// Levels 2 and 3 only. An `h1` is the page title, already in the header above
/// it, and an `h4` is a detail inside a subsection — listing either turns a
/// scannable column into a second copy of the page.
#[derive(Component)]
pub struct Toc {
    pub headings: Vec<Heading>,
}

impl Toc {
    /// The entries worth listing, or `None` when there are too few to be worth a
    /// column at all — a two-heading page is faster to scroll than to index.
    pub fn entries(headings: &[Heading]) -> Option<Vec<Heading>> {
        let entries: Vec<Heading> = headings
            .iter()
            .filter(|heading| matches!(heading.level, 2 | 3))
            .cloned()
            .collect();

        (entries.len() > 1).then_some(entries)
    }

    fn indent(heading: &Heading) -> &'static str {
        if heading.level == 3 { "pl-4" } else { "" }
    }
}
