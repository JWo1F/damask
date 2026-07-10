use rsc::Component;

use crate::model::Service;

/// The fleet table: one row per service, with zebra striping, an SLO-breach
/// marker, and a nested `StatusBadge` per row.
#[derive(Component)]
pub struct ServiceTable<'a> {
    pub services: &'a [Service],
    pub slo_target: f64,
}

impl ServiceTable<'_> {
    /// Row classes: alternating stripe plus a rail on rows missing the target.
    /// Built here so the template stays a single attribute expression.
    pub fn row_class(&self, service: &Service, index: usize) -> String {
        let mut classes = String::new();
        if index % 2 == 1 {
            classes.push_str("alt");
        }
        if service.breaches_slo(self.slo_target) {
            if !classes.is_empty() {
                classes.push(' ');
            }
            classes.push_str("breach");
        }
        classes
    }
}
