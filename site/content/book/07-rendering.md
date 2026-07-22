+++
title = "Writing it out"
summary = "A file on disk, the whitespace features, a test suite, and the traits underneath."
+++

helm prints to stdout, which was fine while the output was one `<span>`. Now it
is a document, and you want it in a file:

```rust
std::fs::write(
    "out.html",
    page.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &dashboard)])),
)
.expect("write out.html");
```

```sh
$ cargo run && open out.html
```

## Reading the output back

Open `out.html` in an editor and the indentation is the indentation of your
templates, spliced together — each one's bytes exactly as written, which means a
`<section>` from a nested component lands at whatever column its own file had it
at. That is fine for a browser and unpleasant for a person.

Two crate features change it, and neither alters the *rendered document* — only
its source:

```toml
damask = { version = "0.2", features = ["pretty"] }
```

- **`pretty`** re-indents the output, so a page you are debugging in "view
  source" is readable.
- **`minify`** collapses it.

Rebuild with `pretty` and the table arrives laid out by its nesting rather than
by its files:

```html
<main class="wrap"><section>
  <h1>Fleet overview</h1>
  <div class="tiles">
    <div class="tile healthy"><div class="n">1</div><div class="k">healthy</div></div>
    …
  </div>
  <table>
    <thead>
      <tr><th>Service</th>…</tr>
    </thead>
```

They are features rather than arguments because `Component::render` builds its
own renderer — there is no call site to pass a choice at, and threading one
through every component would put a formatting concern in every template.

A renderer that lays its output out tracks verbatim regions itself: the content
of `<pre>`, `<textarea>`, `<script>` and `<style>` is left alone, because a space
added there is a space the reader gets. helm's inlined stylesheet survives
`pretty` untouched for that reason.

## Testing what you rendered

A component is a struct with a method returning a `String`, so a test is a
`render()` and an assertion. The interesting assertions are often about what is
**absent**:

```rust
// in src/status_badge.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_badge_carries_its_status_in_the_markup() {
        let out = StatusBadge {
            status: Status::Down,
        }
        .render();
        assert_eq!(out, r#"<span class="badge down" data-status="down">Down</span>"#);
    }
}
```

```rust
// in src/deploy_feed.rs
#[test]
fn an_empty_feed_says_so_and_renders_no_list() {
    let out = DeployFeed {
        deploys: &[],
        limit: 2,
    }
    .render();
    assert!(out.contains("Nothing has shipped"), "{out}");
    assert!(!out.contains("<ul"), "{out}");
}
```

For a component whose content is a slot, build the children with `fragment` and
call `render_with` — the shell can then be tested without the dashboard:

```rust
#[test]
fn the_shell_wraps_whatever_it_is_given() {
    let fleet = demo_fleet();
    let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
    let out = Page {
        title: "Fleet status".into(),
        fleet: &fleet,
        nav: vec!["Overview"],
        current: "Overview",
        commit: "9f3c1ab7d20e".into(),
        year: 2026,
    }
    .render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &body)]));

    assert!(out.starts_with("<!DOCTYPE html>"), "{out:.40}");
    assert!(out.contains(r#"<main class="wrap"><p>hi</p></main>"#), "{out}");
}
```

```sh
$ cargo test
running 3 tests
...
test result: ok. 3 passed; 0 failed
```

Note that the last two assertions are exact strings, so they are sensitive to the
whitespace features. Assert on the markup you care about rather than on a whole
document, or pin the feature set the tests run under.

## What the derive actually gave you

Two traits. `Render` is the low-level one — write yourself into a renderer:

```rust
pub trait Render {
    fn render_into(&self, r: &mut dyn Renderer);
    fn render_slots(&self, r: &mut dyn Renderer, slots: Slots<'_>);
}
```

The derive emits `render_slots` — the lowered template — and points `render_into`
at it with no slots filled. `render_slots` has a default that ignores its slots,
so content with none of its own, like a `Fragment`, stays a one-method impl.

`Component` builds on it with the two methods you have been calling:

```rust
pub trait Component: Render {
    fn default_renderer(&self) -> Box<dyn Renderer>;
    fn render(&self) -> String;
    fn render_with(&self, slots: Slots<'_>) -> String;
}
```

Both are object-safe, so `&dyn Component` and `Vec<Box<dyn Component>>` work —
useful when a page holds a list of blocks whose types differ.

## The Renderer trait

`Renderer` owns the output buffer and the escaping policy, and it is the
extensibility seam: components are compiled against `&mut dyn Renderer`, so any
renderer drives any component, including one from a crate Damask has never heard
of.

```rust
pub trait Renderer {
    fn write_raw(&mut self, s: &str);
    fn write_escaped(&mut self, value: &dyn Display);
    fn finish(self: Box<Self>) -> String;
    // …plus defaulted hooks for text, raw display, layout, and verbatim regions
}
```

Three methods carry the meaning. `write_raw` takes markup — tags and already-safe
content. `write_escaped` backs `{ … }` and applies this renderer's policy.
`write_display_raw` backs `{@html … }`; it is defaulted, but worth overriding on
a buffer-backed renderer so it writes in place rather than allocating a `String`.

Everything else has a default, which is what lets a renderer be a dozen lines:

```rust
/// A renderer that upper-cases everything written to it.
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

Driving one by hand:

```rust
let mut r: Box<dyn damask::Renderer> = Box::new(UpcaseRenderer::default());
badge.render_into(r.as_mut());
let out = r.finish();
```

When a component embeds another, the child writes into the **same** renderer.
That is what keeps escaping consistent: a policy is a property of the output, not
of the component, so `StatusBadge` cannot escape differently depending on who
called it.

Implement `Renderer` yourself to change escaping, target a different sink, or
stream. The built-in `StringRenderer` takes an escape function, which is the
shortest path to a renderer for a non-HTML output:

```rust
use damask::renderers::{StringRenderer, escape_none};

let renderer = StringRenderer::with_escape(escape_none);
```

Whether that is a good idea depends on what you are emitting — `escape_none` is
right for a format with no metacharacters and wrong for almost everything else.
[Renderers and escaping](/docs/renderers/) has the rest of the built-ins.

helm is finished: a document, seven components, a test suite and a file you can
open. The last chapter is about the second application — the conventions that
keep a directory of these honest once there are forty of them.
