//! Language server for RSC (`.rsc`) templates.
//!
//! Capabilities (see [`backend`]): parse diagnostics on open/change, in-tag
//! completion of the paired component's fields and methods, and hover. It pairs
//! a template with its sibling `.rs` file and introspects it with `syn`; there
//! is no project-wide index, so the server is stateless per document.

mod analysis;
mod backend;
mod introspect;

use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(backend::Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
