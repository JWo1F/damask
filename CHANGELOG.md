# Changelog

All notable changes to Damask are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project adheres to
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- **Documentation: async templates on the site.** The feature shipped in 0.4.0
  with nothing about it outside the changelog and the agent skill. Now there is
  a book chapter (*The live fleet*, chapter eight) that arrives at `.await` from
  helm needing a store rather than a fixture, and a reference page
  (*Async templates*) covering the detection rule, the trait pair and its
  blanket impls, the `Send`/`Sync` bounds and what each one is for, and the two
  places `.await` cannot go. Every compiler error quoted in either was taken
  from a real build, and every example compiled before it was written down.

  The neighbouring pages gained what belongs on them rather than repeating it:
  `AsyncComponent`/`AsyncRender`/`RenderFuture` on Traits, the async branch of
  the expansion on The Component derive, `Slots::render_async` and a fill's
  `Sync` on Slots, `fragment_async` and the parameterized-snippet limit on
  Snippets, `Renderer: Send` on Renderers. The README gained an *Async
  templates* section of its own, since it had no mention of the feature at all.

- **The landing page's feature list is set as a specimen sheet.** It gained two
  entries — async, and the renderer seam, which the page had never made a claim
  about — and six equal blocks in a bare two-column grid had stopped being a
  composition. Now each entry hangs a mono numeral in its own column and sits
  under a hairline rule, the section says its name in the mono eyebrow the hero
  already uses, and the feature titles are `h3`s under that `h2` rather than a
  flat run of `h2`s. The rules are also what make the layout indifferent to the
  number of features: a last row of one reads as the end of a table rather than
  as an orphan, which the old grid could not do.

### Fixed

- **Documentation: the install snippets said `0.2`.** Five of them, across the
  README, the landing page, two book chapters and the renderers reference — all
  now `0.5`. Pasting `0.2` got you a version with no async in it at all.

- **The site's header overflowed every page sideways on a phone.** The row wants
  425px of wordmark, sections, search and two icon buttons; a 375px screen
  offers 335px, so every page on the site scrolled horizontally — including ones
  with nothing wide in them. The wordmark is now `sr-only` below `sm`, which is
  the 90px that did it: `hidden` would have taken the home link's only text and
  left it with no accessible name, while `sr-only` keeps it in the accessibility
  tree and out of the layout. Gaps and the section links' padding tighten a
  little below `sm` alongside it. Below 360px the GitHub icon steps out too — it
  is the one control up there that the footer also carries on every page.

## [0.5.0] - 2026-08-30

### Added

- **`damask_template::template_awaits`**, and template resolution moved into
  `damask-template` alongside it (`resolve`, `Resolved`, `component_basename` —
  it was private to `damask-macros`). A framework wrapping
  `#[derive(Component)]` emits code of its own beside the derive's, and what a
  component *is* now differs between the two cases: an awaiting one implements
  `AsyncComponent` and so has no `Component` for that code to hang a response
  impl on. A wrapper macro has no type information to tell them apart, and
  rustc rejects an impl whose `where Self: Component` is trivially false — so
  the question has to be answerable from the struct, which is what this does.

### Changed

- **Breaking: an async render is `Send`, so it can be awaited by a server.**
  `RenderFuture` was `Pin<Box<dyn Future + 'a>>`, which meant a future awaiting
  one was not `Send` — and a work-stealing executor cannot drive a future that
  is not. The practical consequence was that a template written with `.await`
  in it could not be rendered from a request handler at all, which is most of
  what the feature is for. Three bounds make it attainable, each on the thing
  the future actually holds across an `.await`:

  - `Renderer: Send`, for the `&mut dyn Renderer` a render writes through.
  - `Slot`'s content is `&(dyn Render + Sync)` rather than `&dyn Render`, for
    the `Slots` an async template carries past each of its awaits. Deliberately
    *not* a `Sync` supertrait on `Render`: a component that neither fills a slot
    nor awaits anything — a generic one especially — is unaffected.
  - `AsyncRender: Sync`, for the component's own `&self`. It falls out
    automatically for a plain struct of data; a generic component that wants to
    await names `Sync` on the parameters it holds.

  A hand-written `Renderer` over a non-`Send` backing store, or a slot filled
  with non-`Sync` content, is what this can break. Neither is affected by
  anything the derive emits.

### Fixed

- **Documentation: `#[component(crate = …)]` on the site.** The derive page said
  `default` was the only option `#[component]` takes, which stopped being true in
  0.4.0; it now documents `crate` alongside it, as the README does.

## [0.4.0] - 2026-08-30

### Added

- **Async templates.** Write `.await` anywhere a `.dmk` holds Rust — `{ … }`,
  an `{#if}`/`{#for}` condition or iterable, an attribute value — and the
  derive emits an async render path automatically; no attribute to add, and a
  template with no `.await` anywhere stays exactly as sync as before, at no
  extra cost. An async component implements the new `AsyncComponent`/
  `AsyncRender` traits instead of `Component`/`Render`, and renders with
  `.render_async().await`. Every sync `Render` gets `AsyncRender` for free
  (blanket impls in `damask`), so an async template can embed a plain sync
  child at no real cost; the reverse — a sync template embedding an
  async-only child — is a compile error, since there is no sync fallback that
  wouldn't mean blocking on a future inside the caller's own executor.

  Two spots don't support `.await`: a component's slot fill (the children
  between `<Comp>…</Comp>`, which travel to the callee as a plain
  `&dyn Render`), and a `{#snippet}` that takes parameters (its closure has to
  stay callable more than once, which an `async move` capturing a
  non-`Copy` parameter can't guarantee). Both have a one-line rewrite — see
  the "Async templates" section of the component-authoring skill.

  New in `damask`: `AsyncRender`, `AsyncComponent`, `RenderFuture`,
  `AsyncFragment`/`fragment_async`, and `Slots::render_async`. New in
  `damask_template`: `needs_async`.

- **`#[component(crate = ::some::path)]` names the path generated code reaches
  Damask through.** For a framework that re-exports Damask rather than having
  its users depend on it: the default `::damask` resolves against the extern
  prelude and nowhere else, so without this every application would have to
  name `damask` in its own `Cargo.toml` just to write a template. The lowering
  takes the path too — `damask_template::lower_with` and `lower_mapped_with`,
  alongside the unchanged `lower` and `lower_mapped`.

- **`data` expands one value into a run of `data-*` attributes.** The second
  attribute with forms of its own, and the Rails `data: { … }` equivalent:
  `data={self.hooks()}` takes anything implementing the new `DataItem` trait —
  pair lists, `HashMap`, `BTreeMap`, `DataSet`, an `Option` of any of them, or
  your own type — and `data=[…]` merges several sources, with a later mention of
  a key overriding an earlier one. `data={ "controller": "modal" }` is the
  inline map. Values implement `DataValue`, which mirrors `Attr` one level down:
  a `bool` renders a bare `data-open` or nothing, an `Option` renders nothing
  when `None`. Keys are verbatim, not dasherized. A `HashMap` is visited in key
  order so the same state always renders the same attribute order.

  A quoted `data="…"` is untouched and stays an ordinary attribute, which is
  what leaves `<object data="movie.swf">` alone — only `data={…}` and `data=[…]`
  expand. Longhand `data-*` attributes are likewise left on the `Attr` path and
  are not merged into the set.

- **`is_attr_name_safe`**, the check both the `data` keys and the key/value
  `AttrSpread` apply to a name before writing it.

### Fixed

- **A key/value `{...expr}` spread could inject an attribute.** `AttrSpread for
  [(K, V)]` escaped the name, but escaping cannot make a name safe: a key
  holding a space or an `=` ends the name and begins a second attribute, so a
  key derived from state could smuggle in an `onclick`. Such a pair is now
  dropped, and trips a `debug_assert` in a debug build. Names written in a
  template were never affected — the parser does not accept one.

## [0.3.2] - 2026-07-23

### Added

- **Language server: go-to-definition for component attributes and slot fills.**
  Cmd/Ctrl-click on a component attribute now jumps to the struct field it sets,
  and on a `slot="…"` fill to the `<slot>` declaration in the target component's
  template — the same reason hover needed native support: both lower to
  generated setters (or, for slots, to nothing) that rust-analyzer cannot follow.
  Component *names* already resolved through rust-analyzer and still do.

## [0.3.1] - 2026-07-23

### Added

- **Language server: component, attribute, and slot intelligence.** Hovering a
  component attribute now shows the field's type, whether it may be omitted
  (`Option<_>` or `#[component(default)]`), and its doc comment — where before
  rust-analyzer saw only the generated builder setter it lowers to. Slot fills
  autocomplete: typing `slot="…"` on a child of a component offers that
  component's declared slot names, and `slot` itself is offered as an attribute.
  Hover explains a `<slot>` declaration and a `slot="…"` fill (flagging a name
  the component does not declare). Component and prop completions now carry doc
  comments, and props are marked optional in the list.

### Changed

- **Language server: much lower memory use.** A multi-crate workspace now runs a
  single rust-analyzer, rooted at the workspace, rather than one per member crate
  — the main cause of the server's footprint growing several-fold in a workspace.
  A closed template's overlay and HTML skeleton are released rather than kept for
  the session.

## [0.3.0] - 2026-07-23

### Changed

- **Breaking.** Loops are now written as a Rust `for`: `{#for pat in E}…{/for}`
  replaces `{#each E as p}…{/each}`. The header is Rust verbatim — `pat` is any
  pattern and `E` any iterable — so there is no bespoke loop syntax to learn and
  the whole header injects and highlights as the Rust it is. The old `, i` index
  shortcut is gone in favour of Rust's own `.enumerate()`:

  | Before | After |
  |---|---|
  | `{#each &self.items as item}…{/each}` | `{#for item in &self.items}…{/for}` |
  | `{#each &self.xs as x, i}…{/each}` | `{#for (i, x) in self.xs.iter().enumerate()}…{/for}` |
  | `{#each E as (k, v)}…{/each}` | `{#for (k, v) in E}…{/for}` |

  `{#each}` is no longer recognized and is a parse error.

## [0.2.0] - 2026-07-22

### Added

- A template can ask about its own slots. The caller's fills are in scope for
  every `{ … }` tag as `slots`, so `{#if slots.has("actions")}<footer>…</footer>{/if}`
  drops a wrapper the caller gave nothing to render inside — the case a `<slot>`
  fallback cannot express, standing in as it does for the content rather than the
  markup around it. `Slots` gains `has`, `has_default` and `get_default` to go
  with `get`, and `{@render slots.get("actions")}` places a fill by name.
- `Render` is implemented for `&T` and for `Option<T>`, the latter rendering
  nothing when `None` — the rule `Attr` already follows, and what lets
  `{@render slots.get(…)}` stand without a guard around it.

### Changed

- **Breaking.** Slots are filled the way web components fill them: a direct child
  of a component element carrying `slot="x"` goes into the `x` slot, the element
  included, and several children may name the same slot — they land there in the
  order written. `<slot>` is now *only* a placeholder; a named `<slot>` inside a
  component element no longer fills anything. Rewrite
  `<Frame><slot name="footer">© 2026</slot></Frame>` as
  `<Frame><span slot="footer">© 2026</span></Frame>`, and forwarding —
  `<slot name="footer"><slot name="footer"/></slot>` — as
  `<slot name="footer" slot="footer"/>`. A bare `<slot/>` inside a component
  element still forwards the default slot, unchanged. The `slot` attribute is
  consumed rather than rendered, and outside a component element it stays an
  ordinary attribute, so a template can address a browser-side custom element's
  shadow slots.

## [0.1.2] - 2026-07-21

### Changed

- Templates are described on their own terms rather than by comparison to
  another template language, in the README, the crate docs and the agent skill.
  `damask-template`'s keywords follow.
- The licence file is `LICENSE`, the `-MIT` suffix having distinguished it only
  from an Apache copy that no longer exists.
- The Tree-sitter grammar moved to its own repository,
  [tree-sitter-damask](https://github.com/JWo1F/tree-sitter-damask), which is
  what Zed can clone; the Zed extension pins it by revision.

## [0.1.1] - 2026-07-20

### Changed

- Each published crate carries its own README. `damask-macros`, `damask-template`
  and `damask-lsp` previously showed the whole project README on crates.io, which
  told a reader to depend on something other than the crate they were looking at.
- `damask-macros` is described by the derive it actually exports. Its published
  description named a `component!` macro that does not exist.

### Fixed

- Broken intra-doc links: `render_into` is documented on `Render`, which declares
  it, rather than on `Component`.

## [0.1.0] - 2026-07-19

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

[Unreleased]: https://github.com/jwo1f/damask/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/jwo1f/damask/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/jwo1f/damask/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/jwo1f/damask/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/jwo1f/damask/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/jwo1f/damask/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jwo1f/damask/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/jwo1f/damask/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/jwo1f/damask/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jwo1f/damask/releases/tag/v0.1.0
