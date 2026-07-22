use damask::Component;

use crate::view::chrome::Chrome;

/// The markup `site.js` clones, as `<template>` elements.
///
/// A site whose subject is a template engine has no business assembling HTML
/// out of string concatenation in JavaScript. Everything the script needs to
/// create at runtime is written here as markup instead: it is scanned by
/// Tailwind like every other template, it is reviewed as markup, and the script
/// clones a fragment and fills its text rather than building and parsing one.
///
/// Filling by `textContent` also means there is no escaping question at the
/// point of use — a search result's preview is text going into a text node, not
/// a string being spliced into markup.
#[derive(Component)]
pub struct Templates {
    /// Carried in for the idle state's "Start here" list, which is rendered
    /// here rather than assembled by the script.
    pub chrome: Chrome,
}
