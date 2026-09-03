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
   damask = "0.5"
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
omitted ones from its `Default`. A framework that re-exports Damask instead of
having its users depend on it names its own path with
`#[component(crate = my_framework::view)]`, since the default `::damask` resolves
only where `damask` is a direct dependency.

Quoted values interpolate, and on an HTML element `attr={expr}` asks the value's
type how to appear — a `bool` renders a bare attribute or none at all, an
`Option` renders nothing when `None`:

```html
<input title="row {self.n}" disabled={self.locked} placeholder={self.hint}/>
```

`disabled` appears only when `locked`, because in HTML the *presence* of the
attribute is what disables the control — `disabled="false"` disables it too.

### `@tokens` — one value from many parts

`{@tokens(…)}` builds a space-separated value, on whatever attribute it is
written. A `class:` directive still overrules a `class` built this way:

```html
<div class={@tokens(self.extra, "base", "is-open": self.open)} class:base={!self.bare}>
<a rel={@tokens("noopener", external: self.away)}>
```

A positional entry is a name, a list of them, or an `Option` — a literal `None`
is dropped at compile time (a bare `None` has no type to infer). A `name: cond`
entry is there while `cond` holds; the name is a bare identifier or, for
anything an identifier cannot spell, a string: `"md:px-3": cond`. Names are
deduplicated and keep their first-mention order, and an empty result omits the
attribute.

> **CSS scanners and `class:`.** A directive puts the class name in the
> *attribute name* (`class:animate-pulse`), where Tailwind and friends do not
> look — the rule gets compiled out of your stylesheet. When a class has to be
> discoverable by a scanner, write it in a helper, whose names are ordinary
> strings: `class={@tokens("animate-pulse": cond)}`.

### `@attrs` — many attributes from one

`{@attrs(…)}` expands into a run of `<name>-*` attributes, the way a Rails view
does with `data: { … }` — and, since the prefix is the attribute it was written
on, the same helper serves `aria` and anything else shaped that way:

```html
<div data={@attrs(self.hooks(), controller: "modal", index: self.i)}
     aria={@attrs(label: self.title)}>
```

A key becomes `<name>-<key>` verbatim — `user_id` is `data-user_id` — and is a
bare identifier or a string, checked when the template compiles, because a name
is not something escaping can make safe. A positional entry contributes whole
pairs and a later one overrides an earlier, keeping the first mention's
position. Values follow the `Attr` rules one level down, so a `bool` renders a
bare `data-open` or nothing, and an `Option` renders nothing when `None`.
`AttrSet` is implemented for pair lists, `HashMap`, `BTreeMap`, `Attrs`,
`Option` of any of them, and whatever you implement it for.

> **No attribute is special.** `data="movie.swf"` and `data={self.url}` are the
> ordinary attribute they look like, on `<object>` as anywhere else; it is the
> helper that expands, not the name.

### Spreading attributes

`{...expr}` splices a prepared run of attributes — for the ones a component
cannot name, such as a computed `data-<controller>-target`, or a map:

```html
<input {...self.wiring} {...&self.data}/>
```

`AttrSpread` is implemented for `&'static str` (markup the author wrote — the
lifetime is what keeps a request-derived value out) and for `[(K, V)]` /
`Vec<(K, V)>`, which escapes and is where anything derived from state belongs.
A tag that writes an attribute itself and also spreads a set holding it writes it
once, from the tag.

A component takes attributes it never declared the same way, given one field to
put them in:

```rust
#[derive(Component, Default)]
#[component(default)]
pub struct Hidden {
    #[prop(rest)]
    pub attrs: Attrs,
}
```

```html
<input type="hidden" {...self.attrs}>       <!-- hidden.dmk -->

<Hidden data-cover-target="input" autofocus/>
```

Which of a tag's attributes are props is settled when it compiles: a declared
prop wins, and everything else goes to the bag. Without a `#[prop(rest)]` field
a component still refuses an attribute it does not name, so a misspelled prop is
a build failure rather than something rendered into the page.

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

## Async templates

Write `.await` anywhere the template holds Rust — a `{ … }` tag, an `{#if}` /
`{#for}` condition or iterable, an attribute value, a snippet body — and the
derive compiles that component to an async render path. There is nothing to add:
it is decided from the template itself, and a template with no `.await` anywhere
is compiled exactly as before, at no cost.

```rust
use damask::Component;

// profile.rs  (paired with profile.dmk)
#[derive(Component)]
pub struct Profile<'a> {
    pub store: &'a Store,
    pub id: UserId,
}
```

```html
<!-- profile.dmk -->
<p>{self.store.load_name(self.id).await}</p>
```

Such a component implements **`AsyncComponent` / `AsyncRender` instead of
`Component` / `Render`**, so it renders with `.render_async().await`:

```rust
use damask::AsyncComponent;

let html = Profile { store: &store, id }.render_async().await;
```

Composition is free in one direction: every sync `Render` gets an `AsyncRender`
whose future has nothing left to poll, so an async template embeds a plain sync
child at no real cost. The other direction is a compile error naming the missing
`Render` — an async component has no sync fallback, because producing one would
mean blocking on a future inside the executor already driving the caller. Make
the enclosing template async too, or load the child's data before constructing
it.

The render future is `Send`, so a request handler can await one. Three bounds
buy that, one per thing held across an `.await`: `Renderer: Send`, a slot fill is
`Sync` (on `Slot`, deliberately not a `Render` supertrait), and
`AsyncRender: Sync` — automatic for a struct of data, and something a *generic*
component that awaits must name on the parameters it holds.

> **Two places `.await` cannot go**, both with a one-line rewrite the compiler
> spells out. A component's **slot fill** reaches the callee as a plain
> `&dyn Render`, so compute the value in a `{ let x = … }` above the tag and pass
> `{x}` in — a `<slot>`'s own fallback body has no such limit. And a `{#snippet}`
> **that takes parameters** cannot `.await` in its body, since its closure has to
> stay callable more than once; await at the call site instead, as
> `{@render row(self.fetch().await)}`.

## Custom renderers

`Renderer` is the extensibility seam — it owns the output buffer and the escaping
policy. Implement it to change escaping or target a different sink; components are
compiled against `&mut dyn Renderer`, so any renderer drives any component. It
requires `Send`, which a buffer-backed renderer satisfies without saying
anything, and which is what lets an async render be awaited on a work-stealing
executor.

## Workspace

| Crate / dir                | Purpose                                             |
|----------------------------|-----------------------------------------------------|
| [`damask`](crates/damask)        | the facade: traits, the HTML renderer, and the derive |
| [`damask-macros`](crates/damask-macros) | the `Component` derive + template resolution |
| [`damask-template`](crates/damask-template) | the `.dmk` parser (shared by macro + LSP) |
| [`tree-sitter-damask`](crates/tree-sitter-damask) | the vendored Tree-sitter grammar, for the website |
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

The Tree-sitter grammar lives in
[crates/tree-sitter-damask/grammar](crates/tree-sitter-damask/grammar), with the
Rust binding the website reads it through in the crate around it. Zed reads the
same directory: [extension.toml](editors/zed/extension.toml) pins a revision of
this repository and points `path` at it, so a grammar change is one commit — and
reaches editors when that commit is pushed and the `rev` is bumped to name it.

The website highlights `.dmk` with that grammar and with the extension's own
queries in [editors/zed/languages/damask](editors/zed/languages/damask) — so a
snippet looks the same on the site as it does in an editor, and editing a query
changes both.

## License

MIT.
