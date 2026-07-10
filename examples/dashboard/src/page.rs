use rsc::{Component, Render};

use crate::model::Fleet;

/// The document shell: doctype, `<head>`, and the `<body>` chrome that wraps
/// every page — masthead above, footer below, caller's content in between.
///
/// `children` is generic rather than `Box<dyn Render>` so the content can borrow
/// the caller's data without boxing.
#[derive(Component)]
pub struct Page<'a, C: Render> {
    pub title: String,
    pub fleet: &'a Fleet,
    pub nav: Vec<&'static str>,
    /// Which entry in `nav` is the current page.
    pub current: &'static str,
    pub commit: String,
    pub year: u32,
    pub children: C,
}
