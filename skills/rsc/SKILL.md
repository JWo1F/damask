---
name: rsc-components
description: >-
  Author and modify RSC (Rust Smart Components): the paired `.rs` + `.rsc`
  component files, the `#[derive(Component)]` struct, template tags (`<%= <%-
  <%+ <% <%#`), composition and children/slots, and custom renderers. Use
  whenever creating or editing a `.rsc` template or a `#[derive(Component)]`
  struct, or wiring RSC into a Rust project.
---

# Authoring RSC components

RSC compiles a template into a `render` method at build time. A component is a
**Rust struct paired with a template file**. Requires Rust ≥ 1.88.

## The two-file rule (most important)

Every component is two files that share a basename and live in the same
directory:

```
button.rs          # the struct (+ methods)
button.html.rsc    # the template
```

- **Always create/edit them as a pair.** Never add a `#[derive(Component)]`
  struct without its `.rsc`, or vice versa.
- **Keep basenames in sync with the struct name:** `struct Button` ↔
  `button.*.rsc` (snake_case). Renaming one means renaming the other.
- **The middle extension is the host language** and picks escaping:
  `.html.rsc` → HTML-escaped, `.js.rsc` / `.css.rsc` → pass-through, `.rsc` →
  plain.

## Defining a component

```rust
// button.rs
use rsc::Component;

#[derive(Component)]
pub struct Button {
    pub label: String,     // fields ARE the props; use as `self.label`
}

impl Button {              // methods are a normal impl, callable as `self.foo()`
    pub fn shout(&self) -> String { format!("{}!", self.label) }
}
```

```html
<!-- button.html.rsc -->
<button><%= self.label %></button>
```

Render it: `Button { label: "OK".into() }.render()` → `<button>OK</button>`.

- The struct is untouched by the derive — generics, other derives, doc comments
  all work.
- Override the template file with `#[template(path = "custom.html.rsc")]`
  (resolved next to the struct).

## Template tags

| Tag           | Meaning                                                        |
|---------------|---------------------------------------------------------------|
| `<%= expr %>` | write `expr`, **escaped** per the host language               |
| `<%- expr %>` | write `expr` **raw** (no escaping)                             |
| `<%+ expr %>` | render a child component / fragment into the output           |
| `<% stmt %>`  | run Rust statement(s) — control flow, `let`, calls            |
| `<%# text %>` | comment (emits nothing)                                        |
| `<%%` / `%%>` | literal `<%` / `%>`                                            |

Control flow spans tags with ordinary Rust braces:

```html
<ul>
<% for item in &self.items { %>
  <li><%= item %></li>
<% } %>
</ul>
```

## Composition and children

Child components and ad-hoc fragments both implement `Render`; `<%+ … %>`
renders any of them into the current buffer (escaping follows the parent).

```rust
#[derive(Component)]
pub struct Card {
    pub button: Button,    // a child component as a field
}
```
```html
<section><%+ self.button %></section>
```

For **children/slots**, take a generic `Render` field and drop it at the slot:

```rust
use rsc::{Component, Render};

#[derive(Component)]
pub struct Layout<C: Render> {
    pub children: C,
}
```
```html
<main><%+ self.children %></main>
```

Pass children as another component, or as a fragment closure via `rsc::fragment`:

```rust
Layout { children: fragment(|r| r.write_raw("<p>hi</p>")) }.render();
// -> <main><p>hi</p></main>
```

Alternatively, the string path: `<%- self.child.render() %>` writes the child's
finished HTML raw (one extra allocation vs `<%+ %>`).

## Custom renderers

The `Renderer` trait owns the output buffer and escaping. Implement it to change
escaping or target a different sink, then drive any component with it —
components are compiled against `&mut dyn Renderer`, not a concrete renderer:

```rust
let mut r: Box<dyn rsc::Renderer> = Box::new(MyRenderer::new());
component.render_into(r.as_mut());
let out = r.finish();
```

## Pitfalls

- **Escaping is host-language-aware.** In `.html.rsc`, `<%= %>` escapes; in
  `.css.rsc` / `.js.rsc` it does not (those renderers pass through — implement a
  custom `Renderer` if you need context-aware escaping there).
- **Duplicate basenames** in the *same* directory are an error; different
  directories are fine (resolution is per-directory). Use `#[template(path)]` to
  disambiguate.
- **Editing a `.rsc` triggers a rebuild** automatically (no build script) — but
  a template edit with no `.rs` change still recompiles the owning crate.
- Struct fields must be visible where you construct the component (use `pub`
  for cross-module use).

## Project setup

Add one dependency; no build script:

```toml
[dependencies]
rsc = "0.1"
```

Import the derive (and trait) with `use rsc::Component;` (or `use
rsc::prelude::*;`).
