use damask::Component;

/// A component with no bag at all, which is every component by default: an
/// attribute it does not declare is a build failure, so a typo cannot reach the
/// page. `tests/ui/rest_without_a_bag.rs` is what that failure looks like.
#[derive(Component, Default)]
#[component(default)]
pub struct Strict {
    pub title: Option<String>,
}
