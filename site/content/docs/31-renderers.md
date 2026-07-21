+++
title = "Renderers and escaping"
summary = "The built-ins, the whitespace features, and writing your own."
section = "Runtime"
+++

## `HtmlRenderer`

The default. HTML-escapes everything that goes through `{ … }`, and is what
`Component::render` builds for an HTML template.

```rust
let mut r: Box<dyn damask::Renderer> = Box::new(damask::HtmlRenderer::new());
component.render_into(r.as_mut());
let out = r.finish();
```

## `StringRenderer`

The generic buffer the built-ins are made of. It takes an escape function, which
is the shortest path to a renderer for a different output format:

```rust
use damask::renderers::{StringRenderer, Whitespace, escape_html, escape_none};

let html = StringRenderer::with_escape(escape_html);
let raw = StringRenderer::with_escape(escape_none).with_whitespace(Whitespace::Pretty);
```

`escape_none` is right for a format with no metacharacters and wrong for almost
everything else.

## Whitespace

`HtmlRenderer::new()` emits each template's bytes exactly as written. Two crate
features change that:

```toml
damask = { version = "0.1", features = ["pretty"] }
```

`pretty`
: Re-indents the output, so "view source" on a page you are debugging is
  readable.

`minify`
: Collapses it.

Neither alters the *rendered document* — only its source. They are features
rather than arguments because `Component::render` builds its own renderer: there
is no call site to pass a choice at, and threading one through every component
would put a formatting concern in every template.

The `Whitespace` enum is the same choice at the type level, for a renderer you
build yourself.

### Verbatim regions

A renderer that lays its output out must leave the content of `<pre>`,
`<textarea>`, `<script>` and `<style>` alone — a space added there is a space the
reader gets. That is what `set_verbatim` is for, and it nests, because such an
element can contain a component containing more of them.

## Writing your own

Implement `Renderer` to change escaping, target a different sink, or stream.
Components are compiled against `&mut dyn Renderer`, so any renderer drives any
component — including ones from other crates.

Three methods are required: `write_raw`, `write_escaped`, `finish`. Implement
`write_display_raw` too if the renderer is buffer-backed, so `{@html … }` writes
in place instead of allocating a `String` first.

## Escaping rules of thumb

- `{ … }` escapes. `{@html … }` does not. Only reach for the second with content
  you produced or that is already escaped — a child's `.render()`, markdown
  compiled at build time.
- Attribute values are escaped by the same policy, including the interpolated
  parts of a quoted value.
- `{...expr}` over a `&'static str` is **not** escaped; over a `[(K, V)]` map it
  **is**. That asymmetry is the whole design: the `'static` bound is what keeps
  request-derived data out of the unescaped path.
