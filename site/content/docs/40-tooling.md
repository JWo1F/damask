+++
title = "Editors and tooling"
summary = "The language server, the Zed extension, and the workspace layout."
section = "Tooling"
+++

## Language server

`damask-lsp` provides diagnostics and completion for `.dmk` files. It shares the
template parser with the macro — `damask-template` — so a diagnostic in the
editor is the same analysis that will fail the build.

```sh
cargo install --path tools/damask-lsp
```

## Zed

The extension in `editors/zed` supplies highlighting and wires up the language
server. The Tree-sitter grammar lives in its own repository,
[tree-sitter-damask](https://github.com/JWo1F/tree-sitter-damask), because Zed
clones a grammar from a repository root; the extension pins it by revision.

## Agent skill

`skills/damask` is a skill for coding agents: the two-file rule, the tag table,
the pitfalls. Point an agent at it before letting it write templates.

## Workspace

| Crate | Purpose |
|---|---|
| `damask` | the facade: traits, the HTML renderer, and the derive |
| `damask-macros` | the `Component` derive and template resolution |
| `damask-template` | the `.dmk` parser, shared by the macro and the LSP |
| `damask-lsp` | the language server |
| `editors/zed` | the Zed extension |
| `examples/showcase` | runnable example components |
| `examples/dashboard` | a full HTML page from seven composed components |

```sh
cargo test --workspace
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
