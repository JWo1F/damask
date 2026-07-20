# Changelog

All notable changes to Damask are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project adheres to
[Semantic Versioning](https://semver.org/).

## [0.1.1] - 2026-07-21

### Changed

- Each published crate carries its own README. `damask-macros`, `damask-template`
  and `damask-lsp` previously showed the whole project README on crates.io, which
  told a reader to depend on something other than the crate they were looking at.
- `damask-macros` is described by the derive it actually exports. Its published
  description named a `component!` macro that does not exist.

### Fixed

- Broken intra-doc links: `render_into` is documented on `Render`, which declares
  it, rather than on `Component`.

## [0.1.0] - 2026-07-21

### Added

- Props a call site may skip. A prop whose type is `Option<_>` may be left out
  of a `<Comp …/>` tag and arrives as `None`; `#[component(default)]` on the
  struct extends that to every prop, filling the skipped ones from the struct's
  `Default`. A required prop left out is still a compile error, and now names
  the prop. A quoted value also reaches an `Option` prop directly
  (`detail="…"` → `Some("…")`), where it previously needed `{Some(…)}`. Spec §7.
- Templates are laid out rather than copied verbatim: a whitespace run
  containing a newline becomes one newline plus the node's nesting depth, so a
  `{# … #}` comment or a `{#if}` tag no longer leaves a blank line in the
  output. `renderers::Whitespace` — and the crate's `pretty` / `minify`
  features — then choose whether the renderer adds the call site's depth to a
  component's markup, or crushes each run to the single space it renders as
  (~14% of a page). Spec §8.5 gives the argument for why none of it can change
  the rendered document.
- `Renderer::write_text`, for the literal text between a template's tags — the
  only markup a renderer may lay out. Defaults to `write_raw`, so a renderer
  that does not format needs no change.
- `Renderer::push_indent` / `pop_indent` / `set_verbatim` / `close_line`, all
  defaulting to no-ops, so the trait stays object-safe and existing renderers
  keep working.

### Changed

- `Slots::render` takes the declaring `<slot>`'s depth, which it applies to a
  fill (written in the caller, laid out from the caller's root) but not to the
  fallback (the declaring template's own markup).

- **Components via `#[derive(Component)]`** on a normal struct (its fields are
  the props). Methods are a plain `impl`; `#[template(path = "…")]` overrides
  the paired template.
- **Brace-tag HTML template language** (`damask-template`), parsed into an
  HTML-aware tree:
  - `{ … }` is a Rust block — prints its value (escaped) or runs as a statement.
  - `{@html}` (raw), `{@render}` (render a snippet/fragment),
    `{#if}`/`{:else if}`/`{:else}`/`{/if}`, `{#each E as p[, i]}`/`{/each}`,
    `{#snippet name(params)}`/`{/snippet}`.
  - `{use path}` — a Rust `use`, scoped to the enclosing HTML element.
  - **HTML and component elements**: lowercase `<div>` renders; capitalized
    `<Component attr={e}>` is built from its attributes and rendered (omitting
    a field that is not skippable is a compile error).
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
- **Attribute values that can decline to render**, via the `Attr` trait.
  `name={expr}` asks the value's type how to appear: `bool` renders a bare
  `disabled` when true and *nothing* when false (an attribute's presence is what
  HTML reads, so `disabled="false"` is a disabled control), and `Option<T>`
  renders nothing for `None`. Implemented for the string types, the numbers,
  `bool` and `Option` of those. There is deliberately no blanket impl over
  `Display`, which would collide with exactly the two impls that matter — a type
  of your own implements `Attr` or reaches the template as a string.
- **Interpolation in quoted attribute values**: `class="px-3 {self.tone()}"`
  holds literal and `{ … }` parts, each escaped. A value with no holes still
  lowers to literal text, so the common case costs nothing.
- **Class lists.** `class` (and only `class`) additionally accepts:
  - `class=[Some("a"), None, "b", { "c": cond }]` — a list whose entries may be
    strings, `Option`s of them, or a map of conditional names. A literal `None`
    is dropped at compile time, since a bare `None` has no type to infer.
  - `class={ "c": cond, "d": cond }` — the map form alone. Told apart from an
    ordinary `class={expr}` by a top-level `:` that is not part of a `::` path.
  - `class:name={cond}` — a directive that adds or removes one name and
    **takes precedence** over whatever the list produced.

  Everything lands in one `ClassList`, which dedupes and keeps first-mention
  order; an empty result omits the attribute.
- **Attribute spreading**: `<button {...expr}>` splices a prepared run of
  attributes, for the ones a component cannot name — a computed name such as
  `data-<controller>-target`, or a map. The `AttrSpread` trait is implemented
  for `&'static str` (markup the author wrote — the lifetime is what keeps a
  request-derived value out) and for `[(K, V)]`/`Vec<(K, V)>`, which escapes and
  is where anything derived from state belongs.
- **Sibling template resolution** via `Span::local_file()` (stable on Rust
  1.88+): `<name>.dmk` is found next to the struct with no build script; editing
  it triggers a rebuild through an emitted `include_bytes!`.
- **`Renderer` trait** — the extensibility seam owning the output buffer and
  escaping policy — with the built-in `HtmlRenderer` (escapes `& < > " '`) and
  the `StringRenderer` core.
- **`Render` trait + composition**: components and `fragment(|r| …)` closures are
  both renderable; `{@render …}` embeds either, and slot content is a borrowed
  `&dyn Render` (or a template `{#snippet}`), so it stays on the caller's stack.
- **`damask-lsp`** language server: parse diagnostics and in-tag completion of a
  component's fields and methods.
- **Zed extension** with a `tree-sitter-damask` grammar (Rust injected into `{ }`
  tags, HTML into text) wired to `damask-lsp`.
- **Agent skill** (`skills/damask`) for authoring components.

Damask is HTML-only: there is no per-language host extension, and `{ … }` always
HTML-escapes.

[Unreleased]: https://github.com/jwo1f/damask/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/jwo1f/damask/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jwo1f/damask/releases/tag/v0.1.0
