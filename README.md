# RSC — Rust Smart Components

React-like, **compile-time** components for Rust. A component is a struct (its
fields are its props) paired with an HTML template that uses a
`{ … }` syntax. The `Component` derive turns the template into a `render` method
at build time, so rendering is plain, allocation-light Rust — no runtime template
engine.

```rust
use rsc::Component;

// greeting.rs  (paired with greeting.rsc)
#[derive(Component)]
pub struct Greeting {
    pub name: String,
}
```

```html
<!-- greeting.rsc -->
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
   rsc = "0.1"
   ```

2. Create a component as **two files that share a basename**, in the same
   directory — `button.rs` and `button.rsc`.

3. `use rsc::Component;`, `#[derive(Component)]` your struct, and call
   `.render()`.

The template is found automatically next to the struct (via `Span::local_file`),
and editing it triggers a rebuild — no `build.rs`, no configuration.

## Template syntax

Templates are HTML with brace-tag tags. A `{ … }` tag holds a **Rust block**:
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
| `{#each E as p}` / `{#each E as p, i}` `…{/each}` | loop |
| `{#snippet name(params)}…{/snippet}` | define a reusable fragment |

```html
<ul>
{#each &self.items as item}
  <li>{item}</li>
{/each}
</ul>
```

Literal braces are written as expressions: `{"{"}`. `<!-- … -->` comments pass
through.

## Elements, components, and slots

Lowercase tags are HTML. **Capitalized tags are components** — built from their
attributes and rendered. Attributes carry Rust: `attr={expr}`, `attr="literal"`,
or bare `attr` (boolean). Omitting a required field is a compile error.

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
    <p>{self.body}</p>                <!-- fills the default slot -->
    <slot name="footer">© {self.year}</slot>
  </Frame>
</div>
```

A component places its slots with `<slot/>`. Slots are not fields — a template
declares as many as it likes without the struct changing, and a `<slot>`'s body
is the fallback rendered when the caller leaves it unfilled:

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

Slots are matched by name at render time, so a misspelled `name` fails silently
rather than at compile time — the price of keeping them off the struct.

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

`{use}` is an ordinary Rust `use` — import components, functions, or anything
else — and it is scoped to the HTML element that encloses it.

## Snippets

**Snippets** are reusable fragments, defined with `{#snippet}` and rendered with
`{@render}`; parameters make them render-props:

```html
{#snippet item(label)}<li>{label}</li>{/snippet}
<ul>{#each &self.labels as label}{@render item(label)}{/each}</ul>
```

Slots can also be filled from Rust, with `render_with`:

```rust
use rsc::{fragment, Component, Renderer, Slot, Slots, DEFAULT_SLOT};

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
| [`rsc`](crates/rsc)        | the facade: traits, the HTML renderer, and the derive |
| [`rsc-macros`](crates/rsc-macros) | the `Component` derive + template resolution |
| [`rsc-template`](crates/rsc-template) | the `.rsc` parser (shared by macro + LSP) |
| [`rsc-lsp`](tools/rsc-lsp) | language server (diagnostics + completion)          |
| [`editors/zed`](editors/zed) | Zed extension (highlighting + LSP)                |
| [`skills/rsc`](skills/rsc) | agent skill for authoring components                |
| [`examples/showcase`](examples/showcase) | runnable example components          |
| [`examples/dashboard`](examples/dashboard) | a full HTML page from 7 composed components |

See [spec.md](spec.md) for the design and [plan.md](plan.md) for the build plan.

## Development

```sh
cargo test --workspace          # runtime, macro, parser, LSP, examples, trybuild
cargo clippy --workspace --all-targets -- -D warnings
( cd editors/zed/grammars/tree-sitter-rsc && tree-sitter test )
```

## License

MIT OR Apache-2.0.
