//! Procedural macros for RSC. Full implementation lands in M3.

use proc_macro::TokenStream;

/// Placeholder for the `component!` macro; real codegen lands in M3.
#[proc_macro]
pub fn component(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
