+++
title = "Rendering and renderers"
summary = "What the derive produces, and the trait that owns escaping and output."
+++

The derive gives your struct two traits.

`Render` is the low-level one — write yourself into a renderer:

```rust
pub trait Render {
    fn render_into(&self, r: &mut dyn Renderer);
    fn render_slots(&self, r: &mut dyn Renderer, slots: Slots<'_>);
}
```

`Component` builds on it with a default renderer and the convenience you
normally call:

```rust
pub trait Component: Render {
    fn default_renderer(&self) -> Box<dyn Renderer>;
    fn render(&self) -> String;
    fn render_with(&self, slots: Slots<'_>) -> String;
}
```

Both are object-safe, so `&dyn Component` and `Vec<Box<dyn Component>>` work.

## The Renderer trait

`Renderer` owns the output buffer and the escaping policy. It is the
extensibility seam: components are compiled against `&mut dyn Renderer`, so any
renderer drives any component.

```rust
pub trait Renderer {
    fn write_raw(&mut self, s: &str);
    fn write_escaped(&mut self, value: &dyn Display);
    fn finish(self: Box<Self>) -> String;
    // …plus defaulted hooks for layout and verbatim regions
}
```

Three methods carry the meaning:

- `write_raw` — markup. Tags and already-safe content.
- `write_escaped` — backs `{ … }`, and applies this renderer's policy.
- `write_display_raw` — backs `{@html … }`.

Everything else has a default, which is what lets a renderer be a dozen lines.

Driving one by hand:

```rust
let mut r: Box<dyn damask::Renderer> = Box::new(MyRenderer::new());
component.render_into(r.as_mut());
let out = r.finish();
```

## Why a child renders through its parent's renderer

When a component embeds another, the child writes into the **same** renderer.
That is what keeps escaping consistent: a policy is a property of the output, not
of the component, so a component cannot escape differently depending on who
called it.

## Whitespace

`HtmlRenderer::new()` emits each template's bytes exactly as written. Two
features change that, and neither alters the *rendered document* — only its
source:

```toml
damask = { version = "0.1", features = ["pretty"] }
```

- **`pretty`** re-indents the output, so a page you are debugging in "view
  source" is readable.
- **`minify`** collapses it.

It is a feature rather than an argument because `Component::render` builds its
own renderer — there is no call site to pass a choice at, and threading one
through every component would put a formatting concern in every template.

Renderers that lay their output out track verbatim regions themselves: the
content of `<pre>`, `<textarea>`, `<script>` and `<style>` is left alone, because
a space added there is a space the reader gets.

## Writing your own

Implement `Renderer` to change escaping, target a different sink, or stream. The
built-in `StringRenderer` takes an escape function, which is the shortest path to
a renderer for a non-HTML output:

```rust
use damask::renderers::{StringRenderer, escape_none};

let renderer = StringRenderer::with_escape(escape_none);
```

Whether that is a good idea depends on what you are emitting — `escape_none` is
right for a format with no metacharacters and wrong for almost everything else.
