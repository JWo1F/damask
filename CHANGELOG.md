# Changelog

All notable changes to RSC are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Components via `#[derive(Component)]`** on a normal struct (its fields are
  the props). Methods are a plain `impl`; `#[template(path = "…")]` overrides
  the paired template.
- **Brace-tag HTML template language** (`rsc-template`), parsed into an
  HTML-aware tree:
  - `{ … }` is a Rust block — prints its value (escaped) or runs as a statement.
  - `{@html}` (raw), `{@render}` (render a snippet/fragment),
    `{#if}`/`{:else if}`/`{:else}`/`{/if}`, `{#each E as p[, i]}`/`{/each}`,
    `{#snippet name(params)}`/`{/snippet}`.
  - `{use path}` — a Rust `use`, scoped to the enclosing HTML element.
  - **HTML and component elements**: lowercase `<div>` renders; capitalized
    `<Component attr={e}>` is built from its attributes and rendered (omitted
    fields are a compile error).
  - **Slots**: `<slot/>` / `<slot name="x">fallback</slot>` render caller-passed
    content, or the slot's own body when unfilled. Slots are not struct fields —
    they travel as a `Slots` argument to `Render::render_slots`, so a template
    can declare any number of them without the struct changing. Callers fill
    them with `<Comp>…</Comp>` and `<slot name="x">…</slot>`, or from Rust with
    `Component::render_with`. A bare `<slot/>` inside a component element stays
    a placeholder, so it forwards the enclosing component's default slot. The
    trade is that names are matched at render time, not compile time.
  - The parser balances nested braces and respects string/char literals, so
    struct literals work inside `{@render …}` and attribute values.
- **Sibling template resolution** via `Span::local_file()` (stable on Rust
  1.88+): `<name>.rsc` is found next to the struct with no build script; editing
  it triggers a rebuild through an emitted `include_bytes!`.
- **`Renderer` trait** — the extensibility seam owning the output buffer and
  escaping policy — with the built-in `HtmlRenderer` (escapes `& < > " '`) and
  the `StringRenderer` core.
- **`Render` trait + composition**: components and `fragment(|r| …)` closures are
  both renderable; `{@render …}` embeds either, and slot content is a borrowed
  `&dyn Render` (or a template `{#snippet}`), so it stays on the caller's stack.
- **`rsc-lsp`** language server: parse diagnostics and in-tag completion of a
  component's fields and methods.
- **Zed extension** with a `tree-sitter-rsc` grammar (Rust injected into `{ }`
  tags, HTML into text) wired to `rsc-lsp`.
- **Agent skill** (`skills/rsc`) for authoring components.

RSC is HTML-only: there is no per-language host extension, and `{ … }` always
HTML-escapes.

[Unreleased]: https://github.com/jwo1f/rsc
