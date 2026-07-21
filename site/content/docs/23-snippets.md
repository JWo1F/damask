+++
title = "Snippets and fragments"
summary = "Reusable pieces inside a template, and closures as renderable content."
section = "Components"
+++

## Snippets

```dmk
{#snippet item(label)}<li>{label}</li>{/snippet}
<ul>{#each &self.labels as label}{@render item(label)}{/each}</ul>
```

Defined with `{#snippet name(params)}…{/snippet}`, rendered with `{@render}`.
Parameters make a snippet a render-prop.

Snippets lower to `let` bindings, which has two consequences:

- A snippet must be **defined before it is used**.
- It follows ordinary Rust scoping, so a snippet defined inside an element is not
  visible after that element closes.

`{@render}` renders snippets and fragments. A component is called with its tag —
`<Chip label="…"/>` — not with `{@render}`.

## Fragments

`damask::fragment` wraps a `Fn(&mut dyn Renderer)` closure as `Render`:

```rust
use damask::{Render, Renderer, fragment};

let kids = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
let mut buf: Box<dyn Renderer> = Box::new(damask::HtmlRenderer::new());
kids.render_into(buf.as_mut());
assert_eq!(buf.finish(), "<p>hi</p>");
```

`Fragment<F>` is the type; `fragment(f)` is the constructor. It exists as an
explicit wrapper because a blanket `impl<F: Fn(..)> Render for F` would conflict
under coherence with the per-component `impl Render`.

Fragments are what slot content desugars to, and how you pass children from Rust
— see [Slots](/docs/slots/#filling-from-rust).

## `as_display`

`damask::as_display` widens a reference to `&dyn Display`. Generated code routes
every `{ … }` and `{@html … }` value through it rather than unsizing at the call
site, which keeps inference working for snippet parameters whose type is not yet
known.

You will not normally call it, with one exception worth knowing: `T` is `Sized`,
so an already-unsized value needs a reference of its own — write `{&*boxed}`
rather than `{*boxed}`.
