# damask-lsp

The language server for [Damask](https://github.com/jwo1f/damask) `.dmk`
templates — compile-time components for Rust.

```sh
cargo install damask-lsp
```

The binary must be on your `PATH`; editor extensions launch it from there.

## How it works

`damask-lsp` does not reimplement Rust or HTML intelligence. It proxies the real
language servers:

- **Rust inside `{ … }` tags** is lowered to Rust and appended to the
  component's paired `.rs` file, which is fed to a child **rust-analyzer**.
  Hover, completion, go-to-definition, and type diagnostics come back mapped
  through source maps to their true positions in the template.
- **The surrounding markup** is projected to an HTML skeleton and forwarded to
  an **HTML language server** for tag and attribute intelligence.

This is why the editor and the compiler agree about your template: both parse it
with [`damask-template`](https://crates.io/crates/damask-template), and the Rust
is checked by the same analyzer that checks the rest of your crate.

## Downstream servers

Both are optional — features degrade gracefully, falling back to static
`syn`-based field and method completion when a server is missing.

| Server | Install | Gives you |
|---|---|---|
| rust-analyzer | `rustup component add rust-analyzer` | Rust hover, completion, go-to-definition, diagnostics inside tags |
| vscode-html-language-server | from `vscode-langservers-extracted` | markup tag and attribute intelligence |

## Editors

A [Zed](https://zed.dev) extension lives in the repository under
`editors/zed/`, providing syntax highlighting, injections, and indentation
alongside this server.

Any LSP-capable editor can use `damask-lsp` directly by launching it for files
matching `*.dmk`; it speaks standard LSP over stdio.

## License

MIT.
