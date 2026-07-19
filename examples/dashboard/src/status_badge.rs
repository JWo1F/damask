use damask::Component;

use crate::model::Status;

/// A status pill. Both the styling hook and the text come from the enum, so a
/// new [`Status`] variant flows through without touching the template.
#[derive(Component)]
pub struct StatusBadge {
    pub status: Status,
}

impl StatusBadge {
    pub fn class(&self) -> String {
        format!("badge {}", self.status.slug())
    }
}
