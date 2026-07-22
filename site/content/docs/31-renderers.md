+++
title = "Renderers and escaping"
summary = "The built-ins, the whitespace features, and writing your own."
section = "Runtime"
+++

## `HtmlRenderer`

The default, and what `Component::render` builds for an HTML template. It
HTML-escapes everything that goes through `{ … }` and leaves `{@html … }` alone.

```rust
let mut r: Box<dyn damask::Renderer> = Box::new(damask::HtmlRenderer::new());
component.render_into(r.as_mut());
let out = r.finish();
```

| Item | Purpose |
|---|---|
| `HtmlRenderer::new()` | the whitespace policy the crate features chose |
| `HtmlRenderer::with_whitespace(ws)` | that choice made explicitly |
| `.as_str()` | borrow the output built so far |
| `finish()` | consume a `Box<Self>` and return the `String` |

It is a thin newtype over `StringRenderer`, and derives `Debug`, `Clone` and
`Default`.

## `StringRenderer`

The generic `String`-backed buffer the built-ins are made of, and the shortest
path to a renderer for a different output format:

```rust
use damask::renderers::{StringRenderer, Whitespace, escape_html, escape_none};

let html = StringRenderer::with_escape(escape_html);
let raw = StringRenderer::with_escape(escape_none).with_whitespace(Whitespace::Pretty);
```

An escape policy is a `pub type EscapeFn = fn(&str, &mut String)`, which appends
to the output rather than returning — so text with no special characters costs a
single `push_str`. Two are provided:

`escape_html`
: Replaces `&`, `<`, `>`, `"` and `'` with their entities, copying runs of
  ordinary characters in bulk.

`escape_none`
: Copies through unchanged. Right for a format with no metacharacters, wrong for
  almost everything else.

Alongside `with_escape` and `with_whitespace`, `StringRenderer` offers `as_str()`
and `into_string()`. Escaped values are formatted straight into the buffer
through the policy, without a full intermediate `String`.

## Whitespace

`HtmlRenderer::new()` emits each template's bytes exactly as written. Two crate
features change that:

```toml
damask = { version = "0.2", features = ["pretty"] }
```

`pretty`
: Re-indents the output — two spaces per level — so "view source" on a page you
  are debugging is readable.

`minify`
: Replaces each newline and the indentation after it with the single space it
  already renders as.

Neither alters the *rendered document*, only its source. Every variant produces
the same page: HTML collapses a run of whitespace containing a newline to one
space wherever whitespace is insignificant, so resizing such a run cannot change
what a browser draws. None of them ever adds a newline where the template had
none, or removes one that separated two things written on separate lines.

They are features rather than arguments because `Component::render` builds its
own renderer: there is no call site to pass a choice at, and threading one
through every component would put a formatting concern in every template. Cargo
features are additive, so `minify` wins when both are enabled — one dependency
asking for `pretty` must not un-minify another's release build.

The `Whitespace` enum is the same choice at the type level, for a renderer you
build yourself:

| Variant | Effect |
|---|---|
| `AsWritten` | the templates' own bytes; each component starts at column 0 |
| `Pretty` | indent each component's markup to the depth of its call site |
| `Minified` | one space per newline run |

`Whitespace::default()` reads the features, which is what `new()` uses.

### What layout never touches

An escaped value and a `{@html … }` value are *data*: a newline inside a hostname
or a log line belongs to the value, and moving it would be the renderer editing
content. So is a tag's own bytes, whose only possible newline is inside an
attribute value.

### Verbatim regions

A renderer that lays its output out must leave the content of `<pre>`,
`<textarea>`, `<script>` and `<style>` alone — a space added there is a space the
reader gets. That is what `set_verbatim` is for, and it nests, because such an
element can contain a component containing more of them.

## Writing your own

Implement `Renderer` to change escaping, target a different sink, or stream.
Components are compiled against `&mut dyn Renderer`, so any renderer drives any
component — including ones from other crates.

Three methods are required: `write_raw`, `write_escaped`, `finish`. Everything
else has a default, so the smallest useful renderer is those three:

```rust
use damask::Renderer;
use std::fmt::Display;

#[derive(Default)]
pub struct UpcaseRenderer {
    buf: String,
}

impl Renderer for UpcaseRenderer {
    fn write_raw(&mut self, s: &str) {
        self.buf.push_str(&s.to_uppercase());
    }

    fn write_escaped(&mut self, value: &dyn Display) {
        self.buf.push_str(&value.to_string().to_uppercase());
    }

    fn finish(self: Box<Self>) -> String {
        self.buf
    }
}
```

Implement `write_display_raw` too if the renderer is buffer-backed, so
`{@html … }` writes in place instead of allocating a `String` first, and the
layout hooks if it formats. See [Traits](/docs/traits/#renderer) for what each
one is for.

## Escaping rules of thumb

- `{ … }` escapes. `{@html … }` does not. Only reach for the second with content
  you produced or that is already escaped — a child's `.render()`, markdown
  compiled at build time.
- Attribute values are escaped by the same policy, including the interpolated
  parts of a quoted value, and both halves of a spread map.
- `{...expr}` over a `&'static str` is **not** escaped; over a `[(K, V)]` map it
  **is**. That asymmetry is the whole design: the `'static` bound is what keeps
  request-derived data out of the unescaped path.
