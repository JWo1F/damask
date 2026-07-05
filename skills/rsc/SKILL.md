---
name: rsc-components
description: >-
  Author and modify RSC (Rust Smart Components): the paired `.rs` + `.rsc`
  component files, the `#[derive(Component)]` struct, the brace template
  tags (`{ }`, `{@html}`, `{@render}`, `{#if}`, `{#each}`, `{#snippet}`,
  `{#use}`), HTML/component elements (`<Foo attr={e}/>`), `<slot>`s, snippets,
  and custom renderers. Use whenever creating or editing a `.rsc` template or a
  `#[derive(Component)]` struct, or wiring RSC into a Rust project.
---

# Authoring RSC components

RSC compiles an HTML template into a `render` method at build time. A component
is a **Rust struct paired with a template file**. Templates use a
`{ … }` syntax and are always HTML. Requires Rust ≥ 1.88.

## The two-file rule (most important)

Every component is two files that share a basename and live in the same
directory:

```
button.rs      # the struct (+ methods)
button.rsc     # the template (HTML)
```

- **Always create/edit them as a pair.** Never add a `#[derive(Component)]`
  struct without its `.rsc`, or vice versa.
- **Keep the basename in sync with the struct name:** `struct Button` ↔
  `button.rsc` (snake_case). Renaming one means renaming the other.
- Template files are just `<name>.rsc` — no `.html` (or other) middle extension.

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
<!-- button.rsc -->
<button>{self.label}</button>
```

Render it: `Button { label: "OK".into() }.render()` → `<button>OK</button>`.

- The struct is untouched by the derive — generics, other derives, doc comments
  all work.
- Override the template file with `#[template(path = "custom.rsc")]` (resolved
  next to the struct).

## Template syntax

A `{ … }` tag is a **Rust block**: an expression prints its value (escaped); a
statement/binding runs and prints nothing.

| Tag | Meaning |
|-----|---------|
| `{ expr }` | print the value, **HTML-escaped** (`{2+3; 10}` prints `10`) |
| `{ let x = e }` / `{ x; }` | binding / statement — runs, no output |
| `{@html expr}` | print `expr` **raw** (no escaping) |
| `{@render expr}` | render a **snippet / fragment** |
| `{#use path}` | a Rust `use`, scoped to the enclosing element |
| `{#if c}…{:else if c2}…{:else}…{/if}` | conditional |
| `{#each E as p}` / `{#each E as p, i}` `…{/each}` | loop |
| `{#snippet name(params)}…{/snippet}` | define a reusable fragment |

`E` is a Rust expression yielding an iterable — usually `&self.items`. Literal
braces are written as an expression: `{"{"}`.

```html
<ul>
{#each &self.items as item}
  <li>{item}</li>
{/each}
</ul>

{#if self.admin}<span class="badge">admin</span>{/if}
```

## Elements, components, and slots

Lowercase tags are HTML; **capitalized tags are components**, built from their
attributes and rendered. Attributes carry Rust: `attr={expr}`, `attr="literal"`,
or bare `attr` (boolean). A missing required field is a **compile error**.
`{#use}` imports (anything — components, functions), scoped to the enclosing
element.

```html
<div>
  {#use crate::widgets::Frame}
  <Frame title={self.heading.clone()}>
    <p>{self.body}</p>                 <!-- default slot -->
    <slot name="footer">© {self.year}</slot>
  </Frame>
</div>
```

A component declares slots as `Render` fields and places them with `<slot>`:

```rust
use rsc::{Component, Render};

#[derive(Component)]
pub struct Frame<Body: Render, Footer: Render> {
    pub title: String,
    pub children: Body,   // <slot/>
    pub footer: Footer,   // <slot name="footer"/>
}
```
```html
<!-- frame.rsc -->
<section><h2>{self.title}</h2><slot/><footer><slot name="footer"/></footer></section>
```

Because attribute values move into the component's fields, borrow with `.clone()`
(e.g. `title={self.heading.clone()}`) when passing a `&self` field.

## Snippets

**Snippets** are reusable fragments — define with `{#snippet}`, render with
`{@render}` (`{@render}` is for snippets/fragments, not components). Parameters
make a snippet a render-prop:

```html
{#snippet item(label)}<li>{label}</li>{/snippet}
<ul>{#each &self.labels as label}{@render item(label)}{/each}</ul>
```

Children can also come from Rust with `rsc::fragment`:

```rust
use rsc::fragment;
Layout { children: fragment(|r| r.write_raw("<p>hi</p>")) }.render();
```

## Custom renderers

The `Renderer` trait owns the output buffer and escaping. Implement it to change
escaping or target a different sink, then drive any component with it:

```rust
let mut r: Box<dyn rsc::Renderer> = Box::new(MyRenderer::new());
component.render_into(r.as_mut());
let out = r.finish();
```

## Pitfalls

- **`{ … }` HTML-escapes; `{@html … }` does not.** Only use `{@html}` for content
  you trust or that is already escaped (e.g. a child's `.render()`).
- **Component attributes move into fields** — pass `attr={self.x.clone()}` for a
  `&self` field, or use `Copy` types. Every non-slot field must be supplied
  (omitting one is a compile error).
- **`<slot>` fields are `Render` generics.** A component with a default slot
  needs a `children` field; `name="x"` needs an `x` field.
- **`{#use}` is scoped** to its enclosing element — an import inside `<div>…</div>`
  is not visible after `</div>`.
- **Snippets must be defined before they are used** (they are `let` bindings).
- **Duplicate basenames** in the *same* directory are an error; different
  directories are fine. Use `#[template(path)]` to disambiguate.
- **Editing a `.rsc` triggers a rebuild** automatically — no build script.
- Struct fields must be visible where you construct the component (use `pub`
  for cross-module use).

## Project setup

```toml
[dependencies]
rsc = "0.1"
```

Import the derive (and trait) with `use rsc::Component;` (or `use
rsc::prelude::*;`).
