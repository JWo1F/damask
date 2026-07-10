use rsc::Component;

use crate::model::Deploy;

/// Recent releases, newest first, with an empty state when nothing has shipped
/// in the window.
#[derive(Component)]
pub struct DeployFeed<'a> {
    pub deploys: &'a [Deploy],
    /// How many entries to show; the rest are summarised as a remainder.
    pub limit: usize,
}

impl DeployFeed<'_> {
    pub fn visible(&self) -> &[Deploy] {
        let n = self.limit.min(self.deploys.len());
        &self.deploys[..n]
    }

    pub fn hidden(&self) -> usize {
        self.deploys.len().saturating_sub(self.limit)
    }
}
