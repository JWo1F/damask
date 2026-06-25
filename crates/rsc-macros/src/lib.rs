//! Procedural macros for **RSC** (Rust Smart Components).
//!
//! This crate provides the [`component!`] macro. It is a private implementation
//! detail of the `rsc` crate, which re-exports the macro; depend on `rsc`, not
//! on this crate directly.

use proc_macro::TokenStream;
use syn::parse_macro_input;

mod codegen;
mod input;
mod resolve;

/// Define a component: a struct paired with a `.rsc` template.
///
/// ```text
/// component! {
///     [visibility] Name
///     [ template = "path"; ]     // optional; else the crate is scanned for `name.<lang>.rsc`
///     [ schema { [pub] field: Type; … } ]
///     [ impl { <inherent items> } ]
/// }
/// ```
///
/// Expands to the struct, an inherent `impl` with the given items, and an
/// `impl rsc::Component` whose `render_into` is generated from the template.
///
/// The template is located at compile time by reading `CARGO_MANIFEST_DIR` and
/// scanning the crate for a file whose name (minus the language extension and
/// `.rsc`) matches the snake-cased component name. No build script is required.
#[proc_macro]
pub fn component(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as input::ComponentInput);
    codegen::expand(parsed).into()
}
