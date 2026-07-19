//! Procedural macros for **RSC** (Rust Smart Components).
//!
//! Provides the `Component` derive. It is a private implementation detail of the
//! `rsc` crate, which re-exports the derive; depend on `rsc`, not on this crate.

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

mod codegen;
mod props;
mod resolve;

/// Derive [`rsc::Component`] for a struct, generating its `render_into` from the
/// paired `.rsc` template.
///
/// ```ignore
/// use rsc::Component;
///
/// // greeting.rs — paired with greeting.rsc
/// #[derive(Component)]
/// pub struct Greeting {
///     pub name: String,
/// }
/// ```
///
/// By default the template is the sibling file `<snake_name>.rsc` in the same
/// directory as the struct (`Greeting` → `greeting.rsc`). Override it with
/// `#[template(path = "…")]`, resolved relative to the struct's source file.
///
/// A prop whose type is `Option<_>` may be left out at a call site; it arrives
/// as `None`. Marking the struct `#[component(default)]` extends that to every
/// prop, filling the ones a call site skips from the struct's `Default` impl.
///
/// The struct is left untouched; only an `impl Component`, the hidden builder
/// call sites construct it through, and a private `include_bytes!` binding that
/// ties the template into the rebuild graph are added.
#[proc_macro_derive(Component, attributes(template, component))]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as DeriveInput);
    // Ask the compiler where this struct lives so we can find its sibling
    // template. Stable since the `proc_macro_span_file` APIs landed; falls back
    // to a crate scan when the span has no local file.
    let source_file = parsed.ident.span().unwrap().local_file();
    codegen::expand(parsed, source_file).into()
}
