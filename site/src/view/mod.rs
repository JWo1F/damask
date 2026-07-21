//! The site's components.
//!
//! Conventions, carried over from the kit in `jwo1f/router`:
//!
//! - **One component per file, named after its file.** Damask resolves a
//!   template by the struct's name, so `struct PageCard` is paired with
//!   `page_card.dmk`.
//! - **Utilities live in the markup, or in the `impl` beside it — never
//!   `@apply`.** A component's whole appearance is readable at its definition,
//!   and Tailwind scans `src/view`, so both halves of a component are equally
//!   visible to it.
//! - **Colours come from theme tokens** (`bg-surface`, `text-ink-soft`), defined
//!   in `ui/app.css`. A raw hex in a template is a palette change that has to be
//!   made twice.
pub mod chrome;
pub mod layouts;
pub mod pages;
pub mod ui;
