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
Parameters make a snippet a render-prop. The name is everything before the first
`(` and the parameters everything up to the last `)`, so a parameter may carry a
type annotation: `{#snippet cell(value: u8)}`.

Snippets lower to `let` bindings, which has two consequences:

- A snippet must be **defined before it is used**.
- It follows ordinary Rust scoping, so a snippet defined inside an element is not
  visible after that element closes.

| Form | Lowers to |
|---|---|
| `{#snippet n()}…{/snippet}` | `let n = fragment(\|r\| { … });` |
| `{#snippet n(p)}…{/snippet}` | `let n = \|p\| fragment(move \|r\| { … });` |

A snippet with no parameters is therefore a `Fragment` value, and one with
parameters is a closure that builds a fresh `Fragment` per call — which is why
the parameters are in scope inside the body and why the same snippet may be
rendered many times.

A snippet's body is laid out from its own root, like a component's, and the depth
of the `{@render}` site is added when it runs.

`{@render}` renders anything that implements `Render`: a snippet, a fragment, a
slot lookup, an `Option` of one, a `Box<dyn Render>`, or a component value. A
component is normally written as a tag instead — `<Chip label="…"/>`.

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

`Fragment` uses `Render`'s default `render_slots`, which ignores the slots and
forwards to `render_into` — a fragment has no `<slot>`s of its own.

Fragments are what slot content desugars to, and how you pass children from Rust
— see [Slots](/docs/slots/#filling-from-rust).

## `as_display`

```rust
pub fn as_display<'a, T: Display + 'a>(value: &'a T) -> &'a dyn Display
```

Widens a reference to `&dyn Display`. Generated code routes every `{ … }` and
`{@html … }` value through it rather than unsizing at the call site: passing
`&(expr)` straight to a `&dyn Display` parameter unsizes whatever type inference
has reached so far, and for a snippet parameter — still an inference variable —
rust-analyzer would resolve that variable to `dyn Display` and then report every
argument at the call site as a mismatch. A generic function makes it a plain
`T: Display` bound, inferred from the call site as usual.

You will not normally call it, with one exception worth knowing: `T` is `Sized`,
which unsizing requires, so an already-unsized value needs a reference of its own
— write `{&*boxed}` rather than `{*boxed}`.
