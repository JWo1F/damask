+++
title = "At a glance"
summary = "The whole language and runtime on one page."
+++

A component is a Rust struct plus a `.dmk` template with the same basename, in
the same directory. The `Component` derive compiles the template into a `render`
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
| `{@render expr}` | render anything that implements `Render` |
| `{use path}` | a Rust `use`, scoped to the enclosing element |
| `{# … #}` | a comment that does not reach the output |
| `{#if c}…{:else if c}…{:else}…{/if}` | conditional |
| `{#for pat in E}…{/for}` | loop (a Rust `for`) |
| `{#snippet name(params)}…{/snippet}` | define a reusable fragment |

A `{ … }` tag is a Rust block. It prints nothing when it ends in `;` or opens
with `let`, `const`, `use`, `fn`, `static`, `type`, `struct`, `enum`, `trait`,
`impl` or `mod`. A literal brace is an expression: `{"{"}`. An `<!-- … -->`
comment passes through to the output.

## Elements

| Form | Meaning |
|---|---|
| `<div>` | HTML element — lowercase tags are HTML |
| `<Card/>` | component — capitalised tags are components |
| `<slot/>`, `<slot name="x">…</slot>` | slot placement, with the body as fallback |
| `slot="x"` on a component's direct child | route that element into the `x` slot |
| `attr={expr}` | the value asks its type how to appear (`Attr`) |
| `attr="text {expr}"` | interpolating string |
| `attr` | boolean attribute, always on |
| `attr={@tokens(…)}` | a space-separated value, assembled from parts (`TokenItem`) |
| `class:name={cond}` | class directive, which wins over whatever `class` produced |
| `attr={@attrs(…)}` | one set expanded into a run of `attr-*` attributes (`AttrSet`) |
| `{...expr}` | attribute spread (`AttrSpread`), HTML elements only |

Void elements — `<br>`, `<input>`, `<img>` and friends — need no end tag. A
self-closing non-void element writes both: `<div/>` renders `<div></div>`. Each
HTML element's content is a Rust scope, so a `{use}` or `{let}` inside one ends
at the end tag.

## Runtime

| Item | Purpose |
|---|---|
| `Component` | `render()`, `render_with(slots)`, `default_renderer()` |
| `Render` | `render_into(r)`, `render_slots(r, slots)` |
| `AsyncComponent` | `render_async()`, `render_with_async(slots)` |
| `AsyncRender` | `render_into_async(r)`, `render_slots_async(r, slots)` |
| `RenderFuture` | the boxed, `Send` future the async methods return |
| `Renderer` | the output buffer and the escaping policy |
| `HtmlRenderer` | the default, HTML-escaping |
| `StringRenderer` | the `String`-backed core, with a swappable escape function |
| `Whitespace` | `AsWritten`, `Pretty`, `Minified` |
| `Slots`, `Slot`, `DEFAULT_SLOT` | filling slots from Rust |
| `fragment(f)`, `Fragment` | a closure as renderable content |
| `fragment_async(f)`, `AsyncFragment` | the same, returning a future |
| `Attr`, `AttrSpread`, `TokenItem`, `TokenList` | how values become attributes |
| `AttrSet`, `IntoAttrValue`, `Attrs` | how one set becomes a run of `name-*` attributes |
| `as_display` | widen a reference to `&dyn Display` |

`use damask::prelude::*;` brings in the common set.

## Async

A template with `.await` anywhere in its own Rust implements `AsyncComponent` /
`AsyncRender` **instead of** `Component` / `Render`, and renders with
`.render_async().await`. Nothing to add — the derive decides from the template.
A sync child embeds in an async parent for free; the reverse is a compile error.
See [Async templates](/docs/async/).

## Derive attributes

| Attribute | Effect |
|---|---|
| `#[template(path = "…")]` | use this template instead of the sibling one |
| `#[component(default)]` | every prop is skippable, filled from `Default` |
| `#[component(crate = ::path)]` | the path generated code reaches Damask through |

## Crate features

| Feature | Effect |
|---|---|
| *(none)* | `Whitespace::AsWritten` — each template's bytes as written |
| `pretty` | `Whitespace::Pretty` — re-indent the output |
| `minify` | `Whitespace::Minified`; wins when both are enabled |

## Requirements

Rust 1.88 or newer, because resolving the sibling template uses
`Span::local_file`. No build script; editing a `.dmk` triggers a rebuild.
