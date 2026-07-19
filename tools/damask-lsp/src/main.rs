//! Language server for Damask (`.dmk`) templates.
//!
//! Rather than reimplement Rust and HTML intelligence, `damask-lsp` acts as a
//! *proxy* to the real language servers (see [`backend`]):
//!
//! - Code inside `{ … }` tags is lowered to Rust ([`virtual_file`]) and appended
//!   to the component's paired `.rs`, which is fed to a child **rust-analyzer**
//!   ([`lsp_client`]). Hover, completion, go-to-definition, and diagnostics come
//!   back mapped to the template.
//! - The surrounding markup is projected to an HTML skeleton ([`html_doc`]) and
//!   forwarded to an **HTML language server** for tag/attribute intelligence.
//!
//! A static [`introspect`]ion of the paired component is kept as a fallback for
//! when the downstream servers are unavailable. Positions are translated with
//! the source maps produced during lowering; [`analysis`] classifies the cursor.

mod analysis;
mod backend;
mod html_doc;
mod introspect;
mod lsp_client;
mod virtual_file;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(backend::Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
