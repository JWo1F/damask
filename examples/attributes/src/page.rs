use damask::Component;

use crate::passthrough::Passthrough;
use crate::strict::Strict;
use crate::wrapper::Wrapper;

/// The call sites, which is where the whole feature is visible.
///
/// Read `page.dmk` next to this: every attribute on a `<Passthrough/>` there is
/// either a prop the component declares or one it has never heard of, and
/// nothing in the markup says which. That is the point — the call site writes
/// HTML attributes, and where each one goes is settled when it compiles.
#[derive(Component)]
pub struct Page {
    pub email: Option<String>,
    /// Attributes assembled in Rust rather than written down, spread with
    /// `{...}`. Values reach the page escaped, so this is where anything derived
    /// from state belongs.
    pub tracking: Vec<(String, String)>,
}
