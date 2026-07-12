//! The page's stylesheet, embedded from `theme.css` at compile time.
//!
//! It is inlined into the document rather than linked because the example has
//! no static-file server — [`Page`](crate::page::Page) writes it through
//! `{@html … }`, which emits it unescaped.
//!
//! It cannot live in a `<style>` block in the template: `.rsc` has no raw-text
//! elements, so a `{` in a rule body would open a tag. Keeping it in its own
//! `.css` file also means editors treat it as CSS.

/// The stylesheet source.
///
/// `include_str!` rather than `include_bytes!`: the renderer writes `&str`, and
/// `&[u8]` is not `Display`. Being a `&'static str` also keeps it usable in
/// const context.
pub const CSS: &str = include_str!("theme.css");
