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
| `{#use path}` | a Rust `use`, scoped to the enclosing element |
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

```html
<div>
  {#use crate::widgets::Frame}        <!-- import, scoped to this <div> -->
  <Frame title={self.heading.clone()}>
    <p>{self.body}</p>                <!-- fills the default slot -->
    <slot name="footer">© {self.year}</slot>
  </Frame>
</div>
```

A component declares its slots as `Render` fields and places them with `<slot/>`:

```rust
use rsc::{Component, Render};

#[derive(Component)]
pub struct Frame<Body: Render, Footer: Render> {
    pub title: String,
    pub children: Body,   // the default slot
    pub footer: Footer,   // the `name="footer"` slot
}
```
```html
<!-- frame.rsc -->
<section><h2>{self.title}</h2><slot/><footer><slot name="footer"/></footer></section>
```

`{#use}` is an ordinary Rust `use` — import components, functions, or anything
else — and it is scoped to the HTML element that encloses it.

## Snippets

**Snippets** are reusable fragments, defined with `{#snippet}` and rendered with
`{@render}`; parameters make them render-props:

```html
{#snippet item(label)}<li>{label}</li>{/snippet}
<ul>{#each &self.labels as label}{@render item(label)}{/each}</ul>
```

Children can also be built from Rust with `rsc::fragment`:

```rust
use rsc::fragment;
Layout { children: fragment(|r| r.write_raw("<p>hi</p>")) }.render();
```

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

See [spec.md](spec.md) for the design and [plan.md](plan.md) for the build plan.

## Development

```sh
cargo test --workspace          # runtime, macro, parser, LSP, examples, trybuild
cargo clippy --workspace --all-targets -- -D warnings
( cd editors/zed/grammars/tree-sitter-rsc && tree-sitter test )
```

## License

MIT OR Apache-2.0.
