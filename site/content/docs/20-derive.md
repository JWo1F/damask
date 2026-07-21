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

## Template resolution

The derive looks for a `.dmk` file whose basename is the **struct's** name in
snake_case, in the **same directory** as the file the struct is declared in.

| Struct | Template |
|---|---|
| `Button` | `button.dmk` |
| `PageCard` | `page_card.dmk` |
| `HTTPStatus` | `http_status.dmk` |

Two components may share a `.rs` file, since resolution is by struct name — that
is how `struct Cell` can live in `table.rs` and still pair with `cell.dmk`. What
is not allowed is two templates with the same basename in one directory.

Editing a template triggers a rebuild. There is no build script.

## `#[template(path = "…")]`

Overrides the resolved name. The path is relative to the struct's file.

```rust
#[derive(Component)]
#[template(path = "button_compact.dmk")]
pub struct CompactButton {
    pub label: String,
}
```

## `#[component(default)]`

Makes **every** prop skippable; a prop a call site omits comes from the struct's
own `Default`.

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

Worth it where the defaults are meaningful rather than zero values. Without it,
skippability is per-prop and expressed by the type — see [Props](/docs/props/).

## What is generated

The struct is left untouched: generics, other derives, visibility and doc
comments all survive. Beside it the derive emits:

- `impl Render` — `render_into` and `render_slots`, the lowered template
- `impl Component` — `default_renderer`, and the defaulted `render` /
  `render_with`

The lowered template is straight-line Rust: `write_raw` over the literals,
`write_escaped` for `{ … }`, ordinary `if` and `for` for the control flow.

## Errors

A missing required prop, a misspelled field and a type mismatch are all compile
errors at the call site, because a component tag becomes a struct literal. A
missing template file is a compile error on the derive.

The exception is slots, which are matched by name at render time — see
[Slots](/docs/slots/).

## Visibility

Struct fields must be visible where the component is constructed. Cross-module
use means `pub` fields, the same as any other struct.
