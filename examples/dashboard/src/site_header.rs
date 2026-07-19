use damask::Component;

use crate::model::Fleet;

/// Masthead: brand, navigation with the current entry marked, and a banner
/// whose wording and styling follow the fleet's worst status.
#[derive(Component)]
pub struct SiteHeader<'a> {
    pub fleet: &'a Fleet,
    pub nav: Vec<&'static str>,
    pub current: &'static str,
}

impl SiteHeader<'_> {
    /// `href` for a nav entry — the landing page lives at `/`, the rest at
    /// their lowercased name.
    pub fn href(&self, entry: &str) -> String {
        if entry == "Overview" {
            "/".to_string()
        } else {
            format!("/{}", entry.to_lowercase())
        }
    }

    pub fn nav_class(&self, entry: &str) -> &'static str {
        if entry == self.current { "active" } else { "" }
    }
}
