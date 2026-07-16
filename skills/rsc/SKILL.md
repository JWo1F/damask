---
name: rsc-components
description: >-
  Author and modify RSC (Rust Smart Components): the paired `.rs` + `.rsc`
  component files, the `#[derive(Component)]` struct, the brace template
  tags (`{ }`, `{@html}`, `{@render}`, `{#if}`, `{#each}`, `{#snippet}`,
  `{use}`), HTML/component elements (`<Foo attr={e}/>`), `<slot>`s, snippets,
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
| `{use path}` | a Rust `use`, scoped to the enclosing element |
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
`{use}` imports (anything — components, functions), scoped to the enclosing
element.

Quoted values **interpolate**: `title="row {self.n}"`. On an HTML element,
`attr={expr}` asks the value's type how to appear (the `Attr` trait):

| Value type | Renders |
|-----------|---------|
| `bool` | a bare ` attr` when true, **nothing** when false |
| `Option<T>` | nothing when `None`, else as `T` |
| `&str` / `String` / numbers / `char` | ` attr="escaped"` |

Use a `bool` for HTML boolean attributes — `disabled={self.locked}`. Never
`disabled="{…}"`: in HTML the attribute's *presence* disables the control, so
`disabled="false"` is still disabled. There is no blanket `Display` impl; a type
of your own implements `Attr` or reaches the template as a string.

### Class lists

`class` — and only `class` — takes three further forms:

```html
<div class="a b"                                  <!-- as ever, interpolating -->
     class=[self.extra, None, "b", { "c": cond }] <!-- list; None is dropped -->
     class:c={cond}>                              <!-- directive; wins over the above -->
```

- List entries are strings, `Option`s of them, or a `{ "name": cond }` map. A
  literal `None` is dropped at compile time (a bare `None` has no type to infer).
- `class={ "c": cond }` is the map alone; it is told from an ordinary
  `class={expr}` by a top-level `:` that is not part of a `::` path.
- `class:name={cond}` adds or removes one name and **takes precedence** over
  whatever the list produced. A bare `class:name` is always on.
- Names dedupe, keep first-mention order, and an empty result omits `class`.

> **CSS scanners and `class:`.** A directive puts the class name in the
> *attribute name* (`class:animate-pulse`), where Tailwind and friends do not
> look — the rule gets compiled out of your stylesheet. When a class has to be
> discoverable by a scanner, use the map form, whose names are ordinary strings:
> `class={ "animate-pulse": cond }`.

### Spreading attributes

`{...expr}` splices a prepared run of attributes — for the ones a component
cannot name (a computed `data-<controller>-target`, or a map):

```html
<input {...self.wiring} {...&self.data}/>
```

`AttrSpread` is implemented for `&'static str` (markup the author wrote; the
lifetime keeps a request-derived value out) and `[(K, V)]`/`Vec<(K, V)>`, which
escapes — use that for anything derived from state. Only on HTML elements: a
component takes named props.

```html
<div>
  {use crate::widgets::Frame}
  <Frame title={self.heading.clone()}>
    <p>{self.body}</p>                 <!-- default slot -->
    <slot name="footer">© {self.year}</slot>
  </Frame>
</div>
```

A component places its slots with `<slot>`. Slots are **not fields** — the
struct carries only props, however many slots the template has. A `<slot>`'s
body is the fallback used when the caller leaves it unfilled:

```rust
use rsc::Component;

#[derive(Component)]
pub struct Frame {
    pub title: String,
}
```
```html
<!-- frame.rsc -->
<section><h2>{self.title}</h2><slot/><footer><slot name="footer">© anon</slot></footer></section>
```

A `<slot>` placed directly inside a component element fills that component's
slot of the same name. A bare `<slot/>` there is still a placeholder, so it
**forwards** — this passes the caller's content straight through to `Frame`:

```html
<!-- shell.rsc -->
<Frame title={self.title.clone()}>
  <slot/>                                        <!-- forward the default slot -->
  <slot name="footer"><slot name="footer"/></slot>  <!-- fill wrapping a placeholder -->
</Frame>
```

From Rust, fill slots with `render_with`:

```rust
use rsc::{fragment, Component, Renderer, Slot, Slots, DEFAULT_SLOT};

let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
Layout.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &body)]));
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

- **Control flow does not work in attribute position.** `<input {#if x}foo{/if}>`
  is a parse error, and attribute *names* are static (no `data-{key}=`). Express
  a conditional attribute with a `bool`/`Option` value instead; for a map of
  `data-*`, build the run in Rust and emit it with `{@html}` in content position.
- **`{ … }` HTML-escapes; `{@html … }` does not.** Only use `{@html}` for content
  you trust or that is already escaped (e.g. a child's `.render()`).
- **Component attributes move into fields** — pass `attr={self.x.clone()}` for a
  `&self` field, or use `Copy` types. Every field must be supplied
  (omitting one is a compile error).
- **Slots are matched by name at render time, not compile time.** A misspelled
  `<slot name="…">` renders the fallback (or nothing) instead of failing to
  compile. Filling a slot the template does not declare is silently ignored.
- **`{use}` is scoped** to its enclosing element — an import inside `<div>…</div>`
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
