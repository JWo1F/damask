# Changelog

All notable changes to RSC are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Components via `#[derive(Component)]`** on a normal struct (its fields are
  the props). Methods are a plain `impl`; `#[template(path = "…")]` overrides
  the paired template.
- **Template language** (`rsc-template`): `<%= %>` (escaped), `<%- %>` (raw),
  `<%+ %>` (render a child), `<% %>` (statement), `<%# %>` (comment), `<%%`/`%%>`
  literals, and opt-in whitespace control (`<%_`, `-%>`, `_%>`).
- **Sibling template resolution** via `Span::local_file()` (stable on Rust
  1.88+): the template is found next to the struct with no build script; editing
  it triggers a rebuild through an emitted `include_bytes!`.
- **`Renderer` trait** — the extensibility seam owning the output buffer and
  escaping policy — with built-in `HtmlRenderer` (escapes `& < > " '`),
  `JsRenderer`, `CssRenderer`, and `PlainRenderer`.
- **`Render` trait + composition**: components and `fragment(|r| …)` closures are
  both renderable; `<%+ %>` embeds either, and children/slots are generic
  `Render` fields.
- **`rsc-lsp`** language server: parse diagnostics and in-tag completion of a
  component's fields and methods.
- **Zed extension** with a `tree-sitter-rsc` grammar (Rust injected into tags,
  host language into text) wired to `rsc-lsp`.
- **Agent skill** (`skills/rsc`) for authoring components.

[Unreleased]: https://github.com/jwo1f/rsc
