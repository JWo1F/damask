# Damask — compile-time components for Rust

React-like, **compile-time** components for Rust. A component is a struct (its
fields are its props) paired with an HTML template that uses a `{ … }` tag
syntax. The `Component` derive turns the template into a `render` method at
build time, so rendering is plain, allocation-light Rust — no runtime template
engine.

```rust
use damask::Component;

// greeting.rs  (paired with greeting.dmk)
#[derive(Component)]
pub struct Greeting {
    pub name: String,
}
```

```html
<!-- greeting.dmk -->
Hello {self.name}!
```

```rust
assert_eq!(Greeting { name: "Ada".into() }.render(), "Hello Ada!");
// `{ … }` HTML-escapes:
assert_eq!(Greeting { name: "<b>".into() }.render(), "Hello &lt;b&gt;!");
```

## Quickstart

1. Add the dependency (no build script, Rust ≥ 1.88):

   ```toml
   [dependencies]
   damask = "0.2"
   ```

2. Create a component as **two files that share a basename**, in the same
   directory — `button.rs` and `button.dmk`.

3. `use damask::Component;`, `#[derive(Component)]` your struct, and call
   `.render()`.

The template is found automatically next to the struct (via `Span::local_file`),
and editing it triggers a rebuild — no `build.rs`, no configuration.

## Template syntax

Templates are HTML with brace tags. A `{ … }` tag holds a **Rust block**:
if it's an expression, its value is printed (HTML-escaped); if it's a statement
or binding, it runs and prints nothing.

| Tag | Meaning |
|-----|---------|
| `{ expr }` | print the block's value, HTML-escaped (`{2+3; 10}` prints `10`) |
| `{ let x = e }` / `{ x; }` | a binding / statement — runs, prints nothing |
| `{@html expr}` | print `expr` raw (unescaped) |
| `{@render expr}` | render a snippet / fragment |
| `{use path}` | a Rust `use`, scoped to the enclosing element |
| `{#if c}…{:else if c2}…{:else}…{/if}` | conditional |
| `{#for pat in E}…{/for}` | loop — a Rust `for` |
| `{#snippet name(params)}…{/snippet}` | define a reusable fragment |

```html
<ul>
{#for item in &self.items}
  <li>{item}</li>
{/for}
</ul>
```

Literal braces are written as expressions: `{"{"}`. `<!-- … -->` comments pass
through.

## Elements, components, and slots

Lowercase tags are HTML. **Capitalized tags are components** — built from their
attributes and rendered. Attributes carry Rust: `attr={expr}`, `attr="literal"`,
or bare `attr` (boolean). Omitting a required field is a compile error naming
it; a field whose type is `Option<_>` may be omitted and arrives as `None`, and
`#[component(default)]` on the struct makes every field skippable, filling the
omitted ones from its `Default`.

Quoted values interpolate, and on an HTML element `attr={expr}` asks the value's
type how to appear — a `bool` renders a bare attribute or none at all, an
`Option` renders nothing when `None`:

```html
<input title="row {self.n}" disabled={self.locked} placeholder={self.hint}/>
```

`disabled` appears only when `locked`, because in HTML the *presence* of the
attribute is what disables the control — `disabled="false"` disables it too.

### Class lists

`class` takes three further forms, and a `class:` directive overrules them all:

```html
<div class=[self.extra, "base", { "is-open": self.open }] class:base={!self.bare}>
```

Entries may be strings, `Option`s of them, or a map of conditional names; a
literal `None` is dropped at compile time (a bare `None` has no type to infer).
Names are deduplicated and keep their first-mention order, and an empty result
omits the attribute.

> **CSS scanners and `class:`.** A directive puts the class name in the
> *attribute name* (`class:animate-pulse`), where Tailwind and friends do not
> look — the rule gets compiled out of your stylesheet. When a class has to be
> discoverable by a scanner, use the map form, whose names are ordinary strings:
> `class={ "animate-pulse": cond }`.

### Spreading attributes

`{...expr}` splices a prepared run of attributes — for the ones a component
cannot name, such as a computed `data-<controller>-target`, or a map:

```html
<input {...self.wiring} {...&self.data}/>
```

`AttrSpread` is implemented for `&'static str` (markup the author wrote — the
lifetime is what keeps a request-derived value out) and for `[(K, V)]` /
`Vec<(K, V)>`, which escapes and is where anything derived from state belongs.

```html
<div>
  {use crate::widgets::Frame}        <!-- import, scoped to this <div> -->
  <Frame title={self.heading.clone()}>
    <p>{self.body}</p>                          <!-- fills the default slot -->
    <span slot="footer">© {self.year}</span>
    <a slot="footer" href="/about">About</a>
  </Frame>
</div>
```

A component places its slots with `<slot/>`, and a caller routes content into a
named one with `slot="…"` on a direct child — the web-component pair. The whole
element goes in, several children may name the same slot (they land in the order
written), and the `slot` attribute itself is consumed rather than rendered.

Slots are not fields — a template declares as many as it likes without the struct
changing, and a `<slot>`'s body is the fallback rendered when the caller leaves
it unfilled:

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

Slots are matched by name at render time, so a misspelled `name` fails silently
rather than at compile time — the price of keeping them off the struct.

`<slot>` is only ever a placeholder, so putting one where a fill goes
**forwards** — it resolves against this component's caller and `slot=` hands the
result to the child:

```html
<!-- shell.dmk -->
<Frame title={self.title.clone()}>
  <slot/>                                   <!-- forward the default slot -->
  <slot name="footer" slot="footer"/>       <!-- forward "footer" -->
</Frame>
```

Outside a component element `slot` is an ordinary attribute, so a template can
still address a browser-side custom element's shadow slots.

A template can also **ask** about its slots: the caller's fills are in scope as
`slots`, which answers what a fallback cannot — whether the markup *around* the
content should exist at all.

```html
<!-- dialog.dmk -->
<div class="dialog">
  <h2>{self.title}</h2>
  {#if slots.has_default()}<p class="body"><slot/></p>{/if}
  {#if slots.has("actions")}<footer>{@render slots.get("actions")}</footer>{/if}
</div>
```

`slots.get(name)` is renderable as it comes — an unfilled slot renders nothing —
so `{@render}` needs no guard of its own; the `{#if}`s above are guarding the
wrappers. `has_default()` / `get_default()` are the same pair for the default
slot.

`{use}` is an ordinary Rust `use` — import components, functions, or anything
else — and it is scoped to the HTML element that encloses it.

## Snippets

**Snippets** are reusable fragments, defined with `{#snippet}` and rendered with
`{@render}`; parameters make them render-props:

```html
{#snippet item(label)}<li>{label}</li>{/snippet}
<ul>{#for label in &self.labels}{@render item(label)}{/for}</ul>
```

Slots can also be filled from Rust, with `render_with`:

```rust
use damask::{fragment, Component, Renderer, Slot, Slots, DEFAULT_SLOT};

let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
Layout.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &body)]));
```

The fills are borrowed, not owned, so slot content stays on the caller's stack
and can borrow the caller's data without boxing.

## Custom renderers

`Renderer` is the extensibility seam — it owns the output buffer and the escaping
policy. Implement it to change escaping or target a different sink; components are
compiled against `&mut dyn Renderer`, so any renderer drives any component.

## Workspace

| Crate / dir                | Purpose                                             |
|----------------------------|-----------------------------------------------------|
| [`damask`](crates/damask)        | the facade: traits, the HTML renderer, and the derive |
| [`damask-macros`](crates/damask-macros) | the `Component` derive + template resolution |
| [`damask-template`](crates/damask-template) | the `.dmk` parser (shared by macro + LSP) |
| [`damask-lsp`](tools/damask-lsp) | language server (diagnostics + completion)          |
| [`editors/zed`](editors/zed) | Zed extension (highlighting + LSP)                |
| [`skills/damask`](skills/damask) | agent skill for authoring components                |
| [`examples/showcase`](examples/showcase) | runnable example components          |
| [`examples/dashboard`](examples/dashboard) | a full HTML page from 7 composed components |

## Development

```sh
cargo test --workspace          # runtime, macro, parser, LSP, examples, trybuild
cargo clippy --workspace --all-targets -- -D warnings
```

The Tree-sitter grammar lives in its own repository,
[tree-sitter-damask](https://github.com/JWo1F/tree-sitter-damask), because Zed
clones a grammar from a repository root. The Zed extension pins it by revision
in [extension.toml](editors/zed/extension.toml).

## License

MIT.
