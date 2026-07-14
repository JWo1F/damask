# RSC — Zed extension

Syntax highlighting and language-server support for [RSC](../../README.md)
`.rsc` templates in the [Zed](https://zed.dev) editor.

## What you get

- **Highlighting** — tag delimiters (`{`, `{#`, `{@`, `{:`, `{/`, `}`) are
  highlighted, the Rust inside `{ … }` tags is highlighted by Zed's Rust grammar
  (injected), and the surrounding markup by the HTML grammar.
- **Language server** — `rsc-lsp` proxies the real language servers rather than
  reimplementing them. Rust inside `{ … }` tags is type-checked by a
  **rust-analyzer** it runs against the paired component, giving true hover,
  completion, go-to-definition, and type diagnostics; the surrounding markup is
  forwarded to an **HTML language server** for tag/attribute intelligence. When a
  downstream server isn't installed it falls back to static (`syn`-based)
  field/method completion.
- **Indentation** — auto-indent on newline, Tab, and re-indent. It comes from
  three places, because no single one covers the whole file: HTML elements
  indent via the injected HTML grammar's own queries, nested brace groups inside
  a tag via [`indents.scm`](languages/rsc/indents.scm), and `{#if}` / `{#each}` /
  `{#snippet}` blocks via the indent regexes in
  [`config.toml`](languages/rsc/config.toml) — those tags are flat sibling nodes
  in the grammar, so a tree-sitter query cannot span them.

## Layout

```
editors/zed/
├── extension.toml            # extension manifest + grammar + LSP registration
├── Cargo.toml, src/lib.rs    # the extension (wasm) that launches rsc-lsp
├── languages/rsc/
│   ├── config.toml           # file association (*.rsc), brackets, indent rules
│   ├── highlights.scm        # delimiter / comment highlighting
│   ├── indents.scm           # indentation for nested brace groups
│   └── injections.scm        # Rust into tags, host language into text
└── grammars/tree-sitter-rsc/ # the Tree-sitter grammar (source of truth)
```

## Install the language server

The extension runs `rsc-lsp`, which must be on your `PATH`:

```sh
cargo install --path tools/rsc-lsp    # from this repo
# or, once published:  cargo install rsc-lsp
```

The extension launches the **installed** binary, not your checkout, and the
language server compiles the template lowering in. So after changing anything in
`tools/rsc-lsp`, `crates/rsc-template`, or `crates/rsc`, reinstall — otherwise
Zed keeps reporting results from the old lowering however many times you restart
the server. `dev-setup.sh` does this for you when the installed copy is stale.

For full intelligence `rsc-lsp` shells out to downstream servers, also on
`PATH` (both optional — features degrade gracefully without them):

- **rust-analyzer** (`rustup component add rust-analyzer`) — Rust hover,
  completion, go-to-definition, and diagnostics inside `{ … }` tags.
- **vscode-html-language-server** (from `vscode-langservers-extracted`) — markup
  tag/attribute completion and hover.

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

**If Zed says "failed to compile grammar 'rsc'"** with `grammar directory …
already exists, but is not a git clone of …`, delete the stale clone Zed made
from a previous run: `rm -rf editors/zed/grammars/rsc` (or re-run
`dev-setup.sh`, which now does this for you), then reinstall.

### Publishing

Push `grammars/tree-sitter-rsc/` (parser `src/` committed) to its own public
repository and set `repository` / `rev` in `extension.toml` to it.

## `zed_extension_api` version

`Cargo.toml` pins `zed_extension_api`; set it to the version matching your
installed Zed (the `language_server_command` shape used here is stable across
recent versions).

## Injection

Rust is injected into `{ … }` tags; HTML into the text around them. `<!-- … -->`
comments are highlighted by the injected HTML grammar. (One edge: a `{` inside an
HTML comment is still treated as a tag.)

## Grammar tests

```sh
cd grammars/tree-sitter-rsc && tree-sitter test
```
