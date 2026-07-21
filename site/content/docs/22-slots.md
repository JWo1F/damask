+++
title = "Slots"
summary = "Placement, fallbacks, forwarding, and filling from Rust."
section = "Components"
+++

A component places caller-supplied content with `<slot>`.

```dmk
<!-- frame.dmk -->
<section>
  <h2>{self.title}</h2>
  <slot/>
  <footer><slot name="footer">© anon</slot></footer>
</section>
```

**Slots are not fields.** A template declares as many as it likes without the
struct changing.

## Fallbacks

A `<slot>`'s body is what renders when the caller leaves it unfilled. `<slot/>`
with no body renders nothing.

## Filling

```dmk
{use crate::ui::Frame}
<Frame title={self.heading.clone()}>
  <p>{self.body}</p>                  <!-- the default slot -->
  <slot name="footer">© {self.year}</slot>
</Frame>
```

Content that is not inside a named `<slot>` fills the default slot, whose name is
the empty string — `DEFAULT_SLOT` — so a named slot can never collide with it.

## Forwarding

A `<slot>` **directly inside a component element** fills that component's slot of
the same name. A bare `<slot/>` there is still a placeholder, so it forwards.

```dmk
<!-- shell.dmk -->
<Frame title={self.title.clone()}>
  <slot/>                                             <!-- forward the default slot -->
  <slot name="footer"><slot name="footer"/></slot>    <!-- fill, wrapping a placeholder -->
</Frame>
```

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
and can borrow the caller's data without boxing.

| Item | Purpose |
|---|---|
| `DEFAULT_SLOT` | the default slot's name — the empty string |
| `Slot::new(name, &render)` | one named fill |
| `Slots::new(&[…])` | the set passed to `render_with` / `render_slots` |
| `Slots::EMPTY` | no fills; what `render()` passes |
| `Slots::get(name)` | the fill for a name, if any |

## Slots vs. props

Use a **prop** for a value the component formats. Use a **slot** for markup the
caller composes.

The distinction has a practical edge: because a label is a slot rather than a
prop, a call site can drop the text at a breakpoint and leave an icon-only
button, wrapping its own `<span class="hidden lg:inline">` — without the
component growing a prop for it.
