+++
title = "Slots"
summary = "Placement, fallbacks, filling with `slot=`, forwarding, asking, and filling from Rust."
section = "Components"
+++

A component places caller-supplied content with `<slot>`, and a caller routes
content into a named slot with `slot="…"` — the same two halves web components
use.

```dmk
<!-- frame.dmk -->
<section>
  <h2>{self.title}</h2>
  <slot/>
  <footer><slot name="footer">© anon</slot></footer>
</section>
```

`<slot>` is **only ever a placeholder**. It marks where content lands; it never
supplies any.

**Slots are not fields.** A template declares as many as it likes without the
struct changing, and without the struct growing a `Render` type parameter for
each.

## Placement

| Form | Places |
|---|---|
| `<slot/>` | the default slot, whose name is the empty string |
| `<slot name="x"/>` | the slot named `x` |
| `<slot name="x">…</slot>` | the same, with the body as fallback |

`name` must be a plain string literal — it is resolved at compile time, so an
interpolated one, which would name a different slot per render, is an error. So
is an empty `name`: write `<slot/>` for the default slot.

## Fallbacks

A `<slot>`'s body is what renders when the caller leaves it unfilled. `<slot/>`
with no body renders nothing. The fallback is the declaring template's own
markup, so it is laid out where it was written; a fill was written in the caller
and is placed at the slot's depth instead.

## Filling

A direct child of a component element carrying `slot="name"` fills that slot.
The element itself goes in — `slot` is a routing instruction, so it is consumed
and never reaches the rendered markup or the child's props. Like `name`, it must
be a plain, non-empty string literal.

```dmk
{use crate::ui::Frame}
<Frame title={self.heading.clone()}>
  <p>{self.body}</p>                          <!-- the default slot -->
  <span slot="footer">© {self.year}</span>
  <a slot="footer" href="/about">About</a>
</Frame>
```

```html
<section>
  <h2>…</h2>
  <p>…</p>
  <footer><span>© 2026</span><a href="/about">About</a></footer>
</section>
```

**Several children may name the same slot.** They land there in the order they
were written, concatenated into one fill — which is why the footer above takes
both the copyright and the link without either needing a wrapper of its own.

Content with no `slot` fills the default slot, whose name is the empty string —
`DEFAULT_SLOT` — so a named slot can never collide with it. Whitespace alone does
not count as content: a component tag whose body is only a line break fills
nothing and takes the cheaper `render_into` path.

Because the element is part of the fill, a named slot always receives markup, not
bare text. Reach for the element the content wants anyway (`<span>`, `<li>`,
`<footer>`); the default slot, which takes children verbatim, is where loose text
belongs.

`slot` is read only off a **direct child of a component element**. Everywhere
else it is an ordinary attribute, so a template can still address the shadow
slots of a browser-side custom element:

```dmk
<my-card><p slot="footer">rendered as slot="footer"</p></my-card>
```

## Forwarding

A component forwards one of its own slots by routing a placeholder: `<slot>`
resolves against *this* component's caller, and `slot=` hands the result to the
child.

```dmk
<!-- shell.dmk -->
<Frame title={self.title.clone()}>
  <slot/>                                   <!-- forward the default slot -->
  <slot name="footer" slot="footer"/>       <!-- forward "footer" -->
</Frame>
```

A bare `<slot/>` needs no `slot=`: with no name to route it to, it is ordinary
default-slot content, so it forwards the default slot and can sit alongside other
markup in the same fill.

## Asking about slots

The caller's fills are in scope as **`slots`**, for any `{ … }` tag. It answers
the question a fallback cannot: a fallback stands in for missing *content*, so it
cannot say whether the markup *around* the content should exist at all.

```dmk
<!-- dialog.dmk -->
<div class="dialog">
  <h2>{self.title}</h2>
  {#if slots.has_default()}<p class="body"><slot/></p>{/if}
  {#if slots.has("actions")}<footer>{@render slots.get("actions")}</footer>{/if}
</div>
```

Unfilled, that renders `<div class="dialog"><h2>…</h2></div>` — no empty `<p>`,
no empty `<footer>`.

Both ways of placing a fill are there: `<slot/>` resolves implicitly, and
`{@render slots.get("actions")}` does it by name. `{@render}` takes the `Option`
as it comes — `impl Render for Option<T>` renders nothing for `None` — so the tag
needs no guard of its own; the `{#if}` above is guarding the `<footer>`, not the
fill.

| Method | Answers |
|---|---|
| `slots.has(name)` | did the caller fill `name`? |
| `slots.get(name)` | `Option<&dyn Render>` for `name` — renderable as-is |
| `slots.has_default()` | did the caller pass any unslotted content? |
| `slots.get_default()` | that content, if any |

`slots` is an ordinary binding, so a template that wants the name for something
else may shadow it; `<slot>` placement does not go through the binding and keeps
working either way.

## Matching is at render time

> [!IMPORTANT]
> Slots are matched **by name, at render time**. A misspelled `name` renders the
> fallback (or nothing) rather than failing to compile, and filling a slot the
> template does not declare is silently ignored.
>
> This is the deliberate cost of keeping slots off the struct. If a slot is
> load-bearing, a test that asserts its content appears is the check the compiler
> is not giving you.

## Filling from Rust

```rust
use damask::{Component, DEFAULT_SLOT, Renderer, Slot, Slots, fragment};

let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
let out = Layout.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &body)]));
```

`Slots::new` takes a slice of `Slot`s, each pairing a name with a `&dyn Render`.
The fills are **borrowed, not owned**: slot content stays on the caller's stack
and can borrow the caller's data without boxing or type erasure. `Slots` is
`Copy` and `Default`, and both constructors are `const`.

| Item | Purpose |
|---|---|
| `DEFAULT_SLOT` | the default slot's name — the empty string |
| `Slot::new(name, &render)` | one named fill |
| `Slots::new(&[…])` | the set passed to `render_with` / `render_slots` |
| `Slots::EMPTY` | no fills; what `render()` and `render_into` pass |
| `Slots::get(name)` / `has(name)` | the fill for a name, if any / whether there is one |
| `Slots::get_default()` / `has_default()` | the same for the default slot |
| `Slots::render(name, r, indent, fallback)` | what a lowered `<slot>` calls |

A name repeated in the slice resolves to the **first** entry. `Slots::render` is
the one method a template's `<slot>` compiles to: it applies `indent` to a fill,
because that markup was laid out from the caller's root, and calls `fallback`
untouched, because that markup is already at the right depth.

`Slots` is the same type a template sees as `slots`, so these are the methods
[Asking about slots](#asking-about-slots) uses.

## Slots vs. props

Use a **prop** for a value the component formats. Use a **slot** for markup the
caller composes.

The distinction has a practical edge: because a label is a slot rather than a
prop, a call site can drop the text at a breakpoint and leave an icon-only
button, wrapping its own `<span class="hidden lg:inline">` — without the
component growing a prop for it.
