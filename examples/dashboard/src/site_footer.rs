use damask::Component;

use crate::model::Fleet;

/// Closing chrome: fleet-wide rollups and build provenance.
#[derive(Component)]
pub struct SiteFooter<'a> {
    pub fleet: &'a Fleet,
    pub commit: String,
    pub year: u32,
}

impl SiteFooter<'_> {
    /// Short commit hash, as a footer would show it.
    pub fn short_commit(&self) -> &str {
        let n = self.commit.len().min(7);
        &self.commit[..n]
    }
}
