# Damask — Zed extension

Syntax highlighting and language-server support for [Damask](../../README.md)
`.dmk` templates in the [Zed](https://zed.dev) editor.

## What you get

- **Highlighting** — tag delimiters (`{`, `{#`, `{@`, `{:`, `{/`, `}`) are
  highlighted, the Rust inside `{ … }` tags is highlighted by Zed's Rust grammar
  (injected), and the surrounding markup by the HTML grammar.
- **Language server** — `damask-lsp` proxies the real language servers rather than
  reimplementing them. Rust inside `{ … }` tags is type-checked by a
  **rust-analyzer** it runs against the paired component, giving true hover,
  completion, go-to-definition, and type diagnostics; the surrounding markup is
  forwarded to an **HTML language server** for tag/attribute intelligence. When a
  downstream server isn't installed it falls back to static (`syn`-based)
  field/method completion.
- **Indentation** — auto-indent on newline, Tab, and re-indent. It comes from
  three places, because no single one covers the whole file: HTML elements
  indent via the injected HTML grammar's own queries, nested brace groups inside
  a tag via [`indents.scm`](languages/damask/indents.scm), and `{#if}` / `{#for}` /
  `{#snippet}` blocks via the indent regexes in
  [`config.toml`](languages/damask/config.toml) — those tags are flat sibling nodes
  in the grammar, so a tree-sitter query cannot span them.

## Layout

```
editors/zed/
├── extension.toml            # extension manifest + grammar + LSP registration
├── Cargo.toml, src/lib.rs    # the extension (wasm) that launches damask-lsp
├── languages/damask/
│   ├── config.toml           # file association (*.dmk), brackets, indent rules
│   ├── highlights.scm        # delimiter / comment highlighting   ─┐ also read
│   ├── indents.scm           # indentation for nested brace groups │ by the
│   └── injections.scm        # Rust into tags, host language into text ┘ website
└── dev-setup.sh              # refreshes the language server, clears Zed's grammar clone
```

The Tree-sitter grammar is not here: it lives in
[tree-sitter-damask](https://github.com/JWo1F/tree-sitter-damask), because Zed
clones a grammar from a repository root. `extension.toml` pins it by revision.

## Install the language server

The extension runs `damask-lsp`, which must be on your `PATH`:

```sh
cargo install damask-lsp                 # from crates.io
# or, from this repo:  cargo install --path tools/damask-lsp
```

The extension launches the **installed** binary, not your checkout, and the
language server compiles the template lowering in. So after changing anything in
`tools/damask-lsp`, `crates/damask-template`, or `crates/damask`, reinstall — otherwise
Zed keeps reporting results from the old lowering however many times you restart
the server. `dev-setup.sh` does this for you when the installed copy is stale.

For full intelligence `damask-lsp` shells out to downstream servers, also on
`PATH` (both optional — features degrade gracefully without them):

- **rust-analyzer** (`rustup component add rust-analyzer`) — Rust hover,
  completion, go-to-definition, and diagnostics inside `{ … }` tags.
- **vscode-html-language-server** (from `vscode-langservers-extracted`) — markup
  tag/attribute completion and hover.

## Installing the extension (dev)

Zed clones the grammar itself from the pinned revision in `extension.toml`, so
there is nothing to stage first:

1. Run `bash editors/zed/dev-setup.sh` to refresh `damask-lsp` and clear Zed's
   grammar clone.
2. In Zed: `zed: install dev extension` → select this `editors/zed/` directory.
3. Open any `.dmk` file.

**If Zed says "failed to compile grammar 'damask'"**, the clone it keeps at
`grammars/damask/` is in its way — either `already exists, but is not a git
clone of …`, or `Your local changes to the following files would be overwritten
by checkout` when the pinned `rev` moves. Run `dev-setup.sh` and reinstall.

Do not reach for `rm -rf grammars/damask` first. That directory is git-ignored,
so nothing ever shows you what is in it, and it is an ordinary clone of the
grammar — which makes it an easy place to edit the grammar by mistake and a
silent place to lose the edit. The second error above is the clone telling you
it holds something. `dev-setup.sh` stops and prints what, rather than deleting
it; `DAMASK_FORCE_GRAMMAR=1` says go ahead once you have looked.

### Changing the grammar

Grammar work happens in
[tree-sitter-damask](https://github.com/JWo1F/tree-sitter-damask). Push there,
then bump `rev` under `[grammars.damask]` in `extension.toml` to adopt it — and
re-vendor `crates/tree-sitter-damask/grammar/` at the same revision, which is
where the website compiles the parser from. The two pins are the same grammar
and drifting them apart is how the editor and the site start disagreeing.

### Changing the queries

`highlights.scm` and `injections.scm` are read by the website's generator as
well as by Zed — see `site/src/highlight.rs`. That is deliberate: a `.dmk`
snippet in the documentation is coloured by the same rules that colour the file
it was taken from. It also means a query change shows up in `cargo test -p
damask-site`, which is the cheapest way to see whether one did what you meant.

## `zed_extension_api` version

`Cargo.toml` pins `zed_extension_api`; set it to the version matching your
installed Zed (the `language_server_command` shape used here is stable across
recent versions).

## Injection

Rust is injected into `{ … }` tags; HTML into the text around them. `<!-- … -->`
comments are highlighted by the injected HTML grammar. (One edge: a `{` inside an
HTML comment is still treated as a tag.)

## Grammar tests

The corpus lives with the grammar:

```sh
git clone https://github.com/JWo1F/tree-sitter-damask
cd tree-sitter-damask && tree-sitter test
```
