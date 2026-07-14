use rsc::Component;

use crate::model::Fleet;

/// The document shell: doctype, `<head>`, and the `<body>` chrome that wraps
/// every page — masthead above, footer below, caller's content in the default
/// slot in between.
#[derive(Component)]
pub struct Page<'a> {
    pub title: String,
    pub fleet: &'a Fleet,
    pub nav: Vec<&'static str>,
    /// Which entry in `nav` is the current page.
    pub current: &'static str,
    pub commit: String,
    pub year: u32,
}
