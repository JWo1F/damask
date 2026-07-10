use rsc::Component;

use crate::model::Fleet;

/// The page's main content: the rollup tiles, the fleet table, and the deploy
/// feed. Kept separate from [`Page`](crate::page::Page) so the shell (doctype,
/// head, chrome) can wrap any content, not just this dashboard.
#[derive(Component)]
pub struct Dashboard<'a> {
    pub fleet: &'a Fleet,
    /// How many deploys the feed shows before summarising the remainder.
    pub feed_limit: usize,
}
