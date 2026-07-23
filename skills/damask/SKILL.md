---
name: damask-components
description: >-
  Author and modify Damask: the paired `.rs` + `.dmk`
  component files, the `#[derive(Component)]` struct, the brace template
  tags (`{ }`, `{@html}`, `{@render}`, `{#if}`, `{#for}`, `{#snippet}`,
  `{use}`), HTML/component elements (`<Foo attr={e}/>`), `<slot>`s, snippets,
  and custom renderers. Use whenever creating or editing a `.dmk` template or a
  `#[derive(Component)]` struct, or wiring Damask into a Rust project.
---

# Authoring Damask components

Damask compiles an HTML template into a `render` method at build time. A component
is a **Rust struct paired with a template file**. Templates use a
`{ … }` tag syntax and are always HTML. Requires Rust ≥ 1.88.

## The two-file rule (most important)

Every component is two files that share a basename and live in the same
directory:

```
button.rs      # the struct (+ methods)
button.dmk     # the template (HTML)
```

- **Always create/edit them as a pair.** Never add a `#[derive(Component)]`
  struct without its `.dmk`, or vice versa.
- **Keep the basename in sync with the struct name:** `struct Button` ↔
  `button.dmk` (snake_case). Renaming one means renaming the other.
- Template files are just `<name>.dmk` — no `.html` (or other) middle extension.

## Defining a component

```rust
// button.rs
use damask::Component;

#[derive(Component)]
pub struct Button {
    pub label: String,     // fields ARE the props; use as `self.label`
}

impl Button {              // methods are a normal impl, callable as `self.foo()`
    pub fn shout(&self) -> String { format!("{}!", self.label) }
}
```

```html
<!-- button.dmk -->
<button>{self.label}</button>
```

Render it: `Button { label: "OK".into() }.render()` → `<button>OK</button>`.

- The struct is untouched by the derive — generics, other derives, doc comments
  all work.
- Override the template file with `#[template(path = "custom.dmk")]` (resolved
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
| `{#for pat in E}…{/for}` | loop — a Rust `for` |
| `{#snippet name(params)}…{/snippet}` | define a reusable fragment |

`{#for pat in E}` is a Rust `for` loop: `pat` is any pattern and `E` any iterable
— usually `&self.items`. An index is `.enumerate()`, exactly as in Rust
(`{#for (i, item) in self.items.iter().enumerate()}`). Literal braces are written
as an expression: `{"{"}`.

```html
<ul>
{#for item in &self.items}
  <li>{item}</li>
{/for}
</ul>

{#if self.admin}<span class="badge">admin</span>{/if}
```

## Elements, components, and slots

Lowercase tags are HTML; **capitalized tags are components**, built from their
attributes and rendered. Attributes carry Rust: `attr={expr}`, `attr="literal"`,
or bare `attr` (boolean). A missing required field is a **compile error** naming
it. `{use}` imports (anything — components, functions), scoped to the enclosing
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

### Skippable props

A prop must be passed unless its **type** says what leaving it out means, and
only `Option<_>` does — a skipped one arrives as `None`:

```rust
#[derive(Component)]
pub struct Notice {
    pub title: String,             // required
    pub detail: Option<String>,    // skippable → None
    pub dismissible: Option<bool>, // a flag a caller may omit
}
```
```html
<Notice title="Deploy finished"/>                    <!-- both skipped → None -->
<Notice title="Rollback" detail="check the log" dismissible/>
```

A quoted value reaches an `Option` prop directly — `detail="…"` and
`detail="row {self.n}"` both arrive as `Some`, with no `Some(…)` written at the
call site. A `{ … }` value stays exact, so forwarding one is
`detail={self.detail.clone()}`.

`#[component(default)]` on the struct makes **every** prop skippable; the ones a
call site omits come from the struct's own `Default`. Worth it where the
defaults are meaningful rather than zero values:

```rust
#[derive(Component)]
#[component(default)]
pub struct Theme { pub accent: String, pub label: String, pub dense: bool }

impl Default for Theme { /* accent: "indigo", label: "Theme", dense: false */ }
```
```html
<Theme/>                        <!-- every prop from Default -->
<Theme label="Compact"/>        <!-- accent and dense from Default -->
```

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
    <p>{self.body}</p>                          <!-- default slot -->
    <span slot="footer">© {self.year}</span>
    <a slot="footer" href="/about">About</a>
  </Frame>
</div>
```

A component places its slots with `<slot>`, and a caller fills a named one with
`slot="…"` on a **direct child** of the component element — the web-component
pair. The element itself is part of the fill, several children may name the same
slot (they land in the order written), and `slot` is consumed rather than
rendered. Content with no `slot` fills the default slot. Outside a component
element `slot` stays an ordinary attribute.

`<slot>` is **only ever a placeholder**; it never supplies content. Slots are
**not fields** — the struct carries only props, however many slots the template
has. A `<slot>`'s body is the fallback used when the caller leaves it unfilled:

```rust
use damask::Component;

#[derive(Component)]
pub struct Frame {
    pub title: String,
}
```
```html
<!-- frame.dmk -->
<section><h2>{self.title}</h2><slot/><footer><slot name="footer">© anon</slot></footer></section>
```

A placeholder that carries `slot=` **forwards**: `name` resolves against this
component's caller, and `slot` hands the result to the child. A bare `<slot/>`
needs no `slot=` — it is default-slot content already.

```html
<!-- shell.dmk -->
<Frame title={self.title.clone()}>
  <slot/>                                   <!-- forward the default slot -->
  <slot name="footer" slot="footer"/>       <!-- forward "footer" -->
</Frame>
```

The caller's fills are in scope as **`slots`** for any `{ … }` tag, which is how
a template guards the markup *around* a slot — the thing a fallback cannot do,
since a fallback replaces the content, not its wrapper:

```html
<!-- dialog.dmk -->
<div class="dialog">
  <h2>{self.title}</h2>
  {#if slots.has_default()}<p class="body"><slot/></p>{/if}
  {#if slots.has("actions")}<footer>{@render slots.get("actions")}</footer>{/if}
</div>
```

`slots.has(name)` / `has_default()` ask; `slots.get(name)` / `get_default()`
return the content, renderable as it comes — `{@render}` on an unfilled slot
writes nothing, so it needs no guard of its own. `slots` is an ordinary binding
and may be shadowed.

From Rust, fill slots with `render_with`:

```rust
use damask::{fragment, Component, Renderer, Slot, Slots, DEFAULT_SLOT};

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
<ul>{#for label in &self.labels}{@render item(label)}{/for}</ul>
```

Children can also come from Rust with `damask::fragment`:

```rust
use damask::fragment;
Layout { children: fragment(|r| r.write_raw("<p>hi</p>")) }.render();
```

## Custom renderers

The `Renderer` trait owns the output buffer and escaping. Implement it to change
escaping or target a different sink, then drive any component with it:

```rust
let mut r: Box<dyn damask::Renderer> = Box::new(MyRenderer::new());
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
  `&self` field, or use `Copy` types. Every field must be supplied unless it is
  skippable (see below); omitting a required one is a compile error.
- **A skippable prop is an `Option`, always.** `bool` is required like anything
  else — write `Option<bool>` for a flag a caller may leave out.
- **A `{ … }` value is the prop's type exactly** — including its `Option`, so
  pass `detail={Some(x)}` or forward `detail={self.detail.clone()}`. Only a
  *quoted* value converts (into the `Option` as well). That exactness is what
  keeps `count={2 + 8}` inferring to the prop's integer type and `&Vec<T>`
  coercing to a `&[T]` prop.
- **Slots are matched by name at render time, not compile time.** A misspelled
  `<slot name="…">` renders the fallback (or nothing) instead of failing to
  compile, and a misspelled `slot="…"` fills a slot the template does not
  declare, which is silently ignored.
- **A named fill always carries an element.** `slot=` lives on the element that
  goes into the slot, so there is no way to drop bare text into a named one —
  wrap it in the element the content wants anyway. Only the default slot takes
  loose children.
- **`{use}` is scoped** to its enclosing element — an import inside `<div>…</div>`
  is not visible after `</div>`.
- **Snippets must be defined before they are used** (they are `let` bindings).
- **Duplicate basenames** in the *same* directory are an error; different
  directories are fine. Use `#[template(path)]` to disambiguate.
- **Editing a `.dmk` triggers a rebuild** automatically — no build script.
- Struct fields must be visible where you construct the component (use `pub`
  for cross-module use).

## Project setup

```toml
[dependencies]
damask = "0.2"
```

Import the derive (and trait) with `use damask::Component;` (or `use
damask::prelude::*;`).
