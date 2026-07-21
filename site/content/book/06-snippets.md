+++
title = "Snippets and fragments"
summary = "Reusable pieces inside one template, and content passed in from Rust."
+++

Sometimes a piece of markup repeats inside a single template, and pulling it out
into a component is more ceremony than it is worth. That is what snippets are
for.

## Snippets

Define with `{#snippet}`, render with `{@render}`:

```dmk
{#snippet chip(label)}<span class="chip">{label}</span>{/snippet}

<div class="row">
  {#each &self.tags as tag}{@render chip(tag)}{/each}
</div>
<div class="row">
  {@render chip("all")}
</div>
```

Parameters make a snippet a render-prop — the caller decides what goes in the
hole:

```dmk
{#snippet row(label, value)}
  <tr><th>{label}</th><td class="num">{value}</td></tr>
{/snippet}

<table>
  {@render row("Uptime", self.uptime.clone())}
  {@render row("Leases", self.leases.to_string())}
</table>
```

> [!NOTE]
> Snippets are `let` bindings under the hood, so a snippet must be **defined
> before it is used**. A `{@render}` above its `{#snippet}` will not compile.

`{@render}` renders snippets and fragments — not components. A component is
called with its tag, `<Chip label="…"/>`.

## Fragments from Rust

`damask::fragment` turns a closure into renderable content:

```rust
use damask::{fragment, Renderer};

let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
```

That is what a template's slot content desugars to, and it is how you pass
children from Rust rather than from markup.

## Filling slots from Rust

```rust
use damask::{Component, DEFAULT_SLOT, Slot, Slots, fragment, Renderer};

let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
let out = Layout.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &body)]));
```

`DEFAULT_SLOT` is the empty string — the name of the slot that `<slot/>` fills —
so a named slot can never collide with it.

The fills are **borrowed, not owned**. Slot content stays on the caller's stack
and can borrow the caller's data without boxing, which is why `Slots::new` takes
a slice of references.

This is mostly what you reach for in tests, where you want to render a component
with known children and assert on the markup:

```rust
#[test]
fn a_plain_button_carries_no_action_attributes() {
    let label = fragment(|r: &mut dyn Renderer| r.write_raw("Cancel"));
    let out = Button { disabled: false }
        .render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &label)]));

    assert!(!out.contains("disabled"), "{out}");
    assert!(out.contains(">Cancel</button>"), "{out}");
}
```
