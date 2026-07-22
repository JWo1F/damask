+++
title = "At a glance"
summary = "The whole language on one page."
+++

A component is a Rust struct plus an HTML template with the same basename, in the
same directory. The `Component` derive compiles the template into a `render`
method.

```rust
use damask::Component;

#[derive(Component)]        // card.rs, paired with card.dmk
pub struct Card {
    pub title: String,
}
```

```dmk
<article class="card"><h3>{self.title}</h3><slot/></article>
```

## Brace tags

| Tag | Meaning |
|---|---|
| `{ expr }` | print the value, **HTML-escaped** |
| `{ let x = e }`, `{ x; }` | binding or statement — runs, prints nothing |
| `{@html expr}` | print `expr` raw |
| `{@render expr}` | render a snippet or fragment |
| `{use path}` | a Rust `use`, scoped to the enclosing element |
| `{# … #}` | a comment that does not reach the output |
| `{#if c}…{:else if c}…{:else}…{/if}` | conditional |
| `{#each E as p}`, `{#each E as p, i}` `…{/each}` | loop |
| `{#snippet name(params)}…{/snippet}` | define a reusable fragment |

A literal brace is an expression: `{"{"}`.

## Elements

| Form | Meaning |
|---|---|
| `<div>` | HTML element |
| `<Card/>` | component — capitalised tags are components |
| `<slot/>`, `<slot name="x">…</slot>` | slot placement, with the body as fallback |
| `slot="x"` on a component's child | route that element into the `x` slot |
| `attr={expr}` | value asks its type how to appear (`Attr`) |
| `attr="text {expr}"` | interpolating string |
| `attr` | boolean attribute, always on |
| `class=[…]` | class list |
| `class:name={cond}` | class directive |
| `{...expr}` | attribute spread (`AttrSpread`), HTML elements only |

## Runtime

| Item | Purpose |
|---|---|
| `Component` | `render()`, `render_with(slots)`, `default_renderer()` |
| `Render` | `render_into(r)`, `render_slots(r, slots)` |
| `Renderer` | the output buffer and escaping policy |
| `HtmlRenderer` | the default, HTML-escaping |
| `Slots`, `Slot`, `DEFAULT_SLOT` | filling slots from Rust |
| `fragment(f)`, `Fragment` | a closure as renderable content |
| `Attr`, `AttrSpread`, `ClassItem`, `ClassList` | how values become attributes |

`use damask::prelude::*;` brings in the common set.

## Requirements

Rust 1.88 or newer. No build script; editing a `.dmk` triggers a rebuild.
