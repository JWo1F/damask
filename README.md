# RSC — Rust Smart Components

React-like, **compile-time** components for Rust. A component is a struct (its
fields are its props) paired with a template. The `Component` derive turns the
template into a `render` method at build time, so rendering is plain,
allocation-light Rust — no runtime template engine.

```rust
use rsc::Component;

// greeting.rs  (paired with greeting.html.rsc)
#[derive(Component)]
pub struct Greeting {
    pub name: String,
}
```

```html
<!-- greeting.html.rsc -->
Hello <%= self.name %>!
```

```rust
assert_eq!(Greeting { name: "Ada".into() }.render(), "Hello Ada!");
// `<%= %>` escapes for the host language:
assert_eq!(Greeting { name: "<b>".into() }.render(), "Hello &lt;b&gt;!");
```

## Quickstart

1. Add the dependency (no build script, Rust ≥ 1.88):

   ```toml
   [dependencies]
   rsc = "0.1"
   ```

2. Create a component as **two files that share a basename**, in the same
   directory — `button.rs` and `button.html.rsc`. The middle extension
   (`html` / `js` / `css`) selects escaping.

3. `use rsc::Component;`, `#[derive(Component)]` your struct, and call
   `.render()`.

The template file is found automatically next to the struct (via
`Span::local_file`), and editing it triggers a rebuild — no `build.rs`, no
manifest, no configuration.

## Template syntax

| Tag           | Meaning                                              |
|---------------|------------------------------------------------------|
| `<%= expr %>` | write `expr`, escaped for the host language          |
| `<%- expr %>` | write `expr` raw (unescaped)                          |
| `<%+ expr %>` | render a child component / fragment into the output  |
| `<% stmt %>`  | run Rust statement(s) — control flow, `let`, calls   |
| `<%# text %>` | comment                                              |
| `<%%` `%%>`   | literal `<%` / `%>`                                   |

```html
<ul>
<% for item in &self.items { %>
  <li><%= item %></li>
<% } %>
</ul>
```

## Composition & children

Child components and fragments both implement `Render`; `<%+ … %>` renders any of
them, inheriting the parent's renderer (so escaping stays correct):

```rust
use rsc::{Component, Render};

#[derive(Component)]
pub struct Layout<C: Render> {   // a slot / children host
    pub children: C,
}
```
```html
<main><%+ self.children %></main>
```

```rust
use rsc::fragment;
Layout { children: fragment(|r| r.write_raw("<p>hi</p>")) }.render();
// -> <main><p>hi</p></main>
```

## Custom renderers

`Renderer` is the extensibility seam — it owns the output buffer and the escaping
policy. Implement it to change escaping or target a different sink; components are
compiled against `&mut dyn Renderer`, so any renderer drives any component.

## Workspace

| Crate / dir                | Purpose                                             |
|----------------------------|-----------------------------------------------------|
| [`rsc`](crates/rsc)        | the facade: traits, renderers, and the derive       |
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
