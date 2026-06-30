# RSC — Zed extension

Syntax highlighting and language-server support for [RSC](../../README.md)
`.rsc` templates in the [Zed](https://zed.dev) editor.

## What you get

- **Highlighting** — tag delimiters (`<%=`, `<%-`, `<%+`, `<%`, `<%#`, `%>`) are
  highlighted, the Rust inside tags is highlighted by Zed's Rust grammar
  (injected), and the surrounding markup by the host-language grammar.
- **Language server** — parse diagnostics, and completion of the paired
  component's fields and methods inside a tag, via `rsc-lsp`.

## Layout

```
editors/zed/
├── extension.toml            # extension manifest + grammar + LSP registration
├── Cargo.toml, src/lib.rs    # the extension (wasm) that launches rsc-lsp
├── languages/rsc/
│   ├── config.toml           # file association (*.rsc), brackets
│   ├── highlights.scm        # delimiter / comment highlighting
│   └── injections.scm        # Rust into tags, host language into text
└── grammars/tree-sitter-rsc/ # the Tree-sitter grammar (source of truth)
```

## Install the language server

The extension runs `rsc-lsp`, which must be on your `PATH`:

```sh
cargo install --path tools/rsc-lsp    # from this repo
# or, once published:  cargo install rsc-lsp
```

## Installing the extension (dev)

Zed loads a Tree-sitter grammar by **cloning a git repository** at a pinned
revision — and the grammar must be at that repo's root. Our grammar lives in a
subdirectory of this monorepo, so it can't be the clone target directly. Run the
setup script once (and again whenever you change `grammar.js`):

```sh
bash editors/zed/dev-setup.sh
```

It regenerates the parser, copies the grammar into a standalone git repo under
`~/.cache/zed-rsc/tree-sitter-rsc`, and rewrites `[grammars.rsc]` in
`extension.toml` to point at it via a `file://` URL. Then:

1. In Zed: `zed: install dev extension` → select this `editors/zed/` directory.
2. Open any `.rsc` file.

> The script's edit to `extension.toml` is machine-specific — **don't commit
> it.** The committed `extension.toml` keeps a GitHub placeholder for publishing.

### Publishing

Push `grammars/tree-sitter-rsc/` (parser `src/` committed) to its own public
repository and set `repository` / `rev` in `extension.toml` to it.

## `zed_extension_api` version

`Cargo.toml` pins `zed_extension_api`; set it to the version matching your
installed Zed (the `language_server_command` shape used here is stable across
recent versions).

## Host-language injection

v1 injects **HTML** into the text between tags — the common case. A `.rsc` file's
true host language is its middle extension (`app.js.rsc`, `theme.css.rsc`);
per-suffix injection (distinct `js.rsc` / `css.rsc` languages sharing this
grammar) is a planned enhancement.

## Grammar tests

```sh
cd grammars/tree-sitter-rsc && tree-sitter test
```
