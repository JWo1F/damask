+++
title = "Editors and tooling"
summary = "The language server, the Zed extension, the agent skill, and the workspace layout."
section = "Tooling"
+++

## Language server

`damask-lsp` speaks standard LSP over stdio, so any editor can launch it for
`*.dmk`.

```sh
cargo install damask-lsp
cargo install --path tools/damask-lsp   # from a checkout
```

| Capability | Notes |
|---|---|
| Completion | trigger characters `.`, `<`, ` `, `:` |
| Hover | |
| Go to definition | |
| Diagnostics | published on change |

It reimplements neither Rust nor HTML. The Rust inside `{ … }` tags is lowered —
by `damask-template`, the same crate and the same function the derive uses — and
appended to the component's paired `.rs` file, which is fed to a child
**rust-analyzer**; answers come back mapped through source maps to their true
positions in the template. The surrounding markup is projected to an HTML
skeleton and forwarded to an **HTML language server**.

That is why the editor and the compiler agree: both parse the template with the
same parser, and the Rust is checked by the same analyzer that checks the rest of
the crate.

| Downstream server | Install | Gives you |
|---|---|---|
| rust-analyzer | `rustup component add rust-analyzer` | hover, completion, go-to-definition, type diagnostics inside tags |
| vscode-html-language-server | from `vscode-langservers-extracted` | tag and attribute intelligence in the markup |

Both are optional. With neither, completion falls back to static `syn`-based
field and method lookup.

## Zed

The extension in `editors/zed` supplies highlighting, injections and indentation
alongside the server. The Tree-sitter grammar lives in its own repository,
[tree-sitter-damask](https://github.com/JWo1F/tree-sitter-damask), because Zed
clones a grammar from a repository root; `extension.toml` pins it by revision.

Block tags are flat in the grammar — `{#if}`, its body and `{/if}` are siblings —
so block indentation is carried by regex rather than by a tree-sitter query,
while HTML elements indent from the injected HTML layer.

## Agent skill

`skills/damask` is a skill for coding agents: the two-file rule, the tag table,
the pitfalls. Point an agent at it before letting it write templates.

## Workspace

| Crate or directory | Purpose |
|---|---|
| `crates/damask` | the facade: traits, the HTML renderer, and the derive |
| `crates/damask-macros` | the `Component` derive and template resolution |
| `crates/damask-template` | the `.dmk` parser and lowering, shared by the macro and the LSP |
| `tools/damask-lsp` | the language server |
| `editors/zed` | the Zed extension |
| `skills/damask` | the agent skill |
| `examples/showcase` | runnable example components |
| `examples/dashboard` | a full HTML page from seven composed components |

```sh
cargo test --workspace          # runtime, macro, parser, LSP, examples, trybuild
cargo clippy --workspace --all-targets -- -D warnings
```

## This site

The site you are reading is `site/` in the same repository — a generator that
reads markdown out of `site/content/`, renders it through Damask components in
`site/src/view/`, and writes static HTML.

```sh
./tools/build-site.sh          # Tailwind, then the generator
./tools/build-site.sh serve    # …and a local server
```

It is worth reading as a worked example of the [page-building
conventions](/book/building-a-page/): a `Chrome` value threaded through the
layouts, a kit under `ui/`, one component per page, and Tailwind pointed at the
whole view tree.
