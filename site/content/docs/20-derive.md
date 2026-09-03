+++
title = "The Component derive"
summary = "Template resolution, attributes, and what the derive generates."
section = "Components"
+++

```rust
use damask::Component;

#[derive(Component)]
pub struct Button {
    pub label: String,
}
```

`damask::Component` names both the trait and the derive, as `serde::Serialize`
does, so one import brings in both.

## Template resolution

The derive asks the compiler where the struct was written — `Span::local_file`,
stable since Rust 1.88 — and looks for a `.dmk` file whose basename is the
**struct's** name in snake_case, in that **same directory**.

| Struct | Template |
|---|---|
| `Button` | `button.dmk` |
| `PageCard` | `page_card.dmk` |
| `HTTPStatus` | `http_status.dmk` |
| `Card2Col` | `card2_col.dmk` |

The name must match exactly: `button.dmk`, never `button.html.dmk`. Two
components may share a `.rs` file, since resolution is by struct name — that is
how `struct Cell` can live in `table.rs` and still pair with `cell.dmk`. What is
not allowed is two templates with the same basename in one directory.

Where the sibling directory yields nothing — a build that could not map the span
to a file, say — the derive falls back to scanning the crate for a matching
basename, ignoring `target/` and dot-directories. Both the sibling lookup and the
scan report an ambiguity rather than guessing.

Editing a template triggers a rebuild, because the derive emits an
`include_bytes!` of the resolved path. There is no build script.

## `#[template(path = "…")]`

Overrides the resolved name. The path is tried relative to the struct's own
directory, then the crate root, then `src/`.

```rust
#[derive(Component)]
#[template(path = "button_compact.dmk")]
pub struct CompactButton {
    pub label: String,
}
```

`path` is the only option `#[template]` takes, and it is required.

## `#[component(default)]`

Makes **every** prop skippable; a prop a call site omits comes from the struct's
own `Default`, which the struct must therefore implement.

```rust
#[derive(Component)]
#[component(default)]
pub struct Theme {
    pub accent: String,
    pub label: String,
    pub dense: bool,
}

impl Default for Theme { /* accent: "indigo", label: "Theme", dense: false */ }
```

```dmk
<Theme/>
<Theme label="Compact"/>
```

The builder starts from `Default::default()` and overwrites what the call site
set, so each skipped prop lands on exactly what `Default` says — nothing is asked
of the field types themselves. Worth it where the defaults are meaningful rather
than zero values. Without it, skippability is per-prop and expressed by the type
— see [Props](/docs/props/).

## `#[prop(rest)]`

On a field rather than the struct, and the only option `#[prop]` takes. It marks
the one field every attribute the call site wrote that is *not* a prop is
collected into:

```rust
#[derive(Component, Default)]
#[component(default)]
pub struct Hidden {
    #[prop(rest)]
    pub attrs: Attrs,
}
```

A struct has at most one, its type is `Attrs`, and it is never required. Without
one, an attribute the component does not declare is a build failure. See
[Attributes a component does not name](/docs/rest-attributes/).

## `#[component(crate = …)]`

Names the path the generated code reaches Damask through. It defaults to
`::damask`, which resolves only where `damask` is a direct dependency of the
crate being compiled — so a framework that re-exports Damask, rather than asking
its users to depend on it by name, has to say which path to use instead.

```rust
mod framework {
    pub use damask as view;
}

#[derive(framework::view::Component)]
#[component(crate = crate::framework::view)]
pub struct Aliased {
    pub who: String,
}
```

The value is a path, not a string, and it stands in for the crate everywhere the
expansion mentions it: the trait impls, the prop builder, and the attribute and
child-component calls the template lowers to. Nothing else changes — the
component renders the same, and a call site writing `<Aliased who="Ada"/>` cannot
tell.

`default` and `crate` are the two options `#[component]` takes, and they may be
given together.

## What is generated

The struct is left untouched: generics, other derives, visibility and doc
comments all survive. Beside it the derive emits:

- `impl Render` — `render_slots` is the lowered template, and `render_into`
  calls it with `Slots::EMPTY`
- `impl Component` — `default_renderer`, returning a boxed `HtmlRenderer`; the
  defaulted `render` and `render_with` come from the trait
- a hidden prop builder that component tags construct the struct through, with
  one setter per field carrying that field's own visibility
- `const _: &[u8] = include_bytes!("…")`, which ties the template into the
  rebuild graph

The lowered template is straight-line Rust: `write_raw` over the literals,
`write_escaped` for `{ … }`, ordinary `if` and `for` for the control flow.

Where the template contains `.await` anywhere in its own Rust, the first two
become `impl AsyncRender` and `impl AsyncComponent` instead — same methods,
returning boxed futures — and no `Render`/`Component` impl is emitted at all.
Nothing else about the expansion changes, including the builder. The derive
decides this from the template, with nothing to declare; see
[Async templates](/docs/async/).

Generics come along: the builder carries the component's own parameters, so
`struct Tagged<'a, T: Display>` takes skippable props like any other. A **tuple
struct** gets no builder — its fields cannot be addressed by name — so it renders
from Rust but cannot be written as a tag.

## Errors

A missing required prop, a misspelled field and a type mismatch are all compile
errors at the call site, because a component tag becomes a builder chain. On the
derive itself: a missing or ambiguous template, an unknown `template` /
`component` option, a template that fails to parse, and Rust inside a `{ … }` tag
that does not parse.

The exception is slots, which are matched by name at render time — see
[Slots](/docs/slots/).

## Visibility

Struct fields must be visible where the component is constructed, because the
generated setters inherit each field's visibility. Cross-module use means `pub`
fields, the same as any other struct.
