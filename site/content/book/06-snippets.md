+++
title = "The rollup tiles"
summary = "Snippets, fragments, and what a template can ask about its own slots."
+++

The dashboard needs a row of rollup tiles above the table: how many services are
healthy, degraded, down, and how many are below target. Written out, that is four
copies of the same six-element block with two words changed:

```dmk
<div class="tiles">
  <div class="tile healthy"><div class="n">{self.fleet.count(Status::Healthy)}</div><div class="k">healthy</div></div>
  <div class="tile degraded"><div class="n">{self.fleet.count(Status::Degraded)}</div><div class="k">degraded</div></div>
  …
</div>
```

A component would fix it, at the cost of a fifth pair of files and a struct for
something that exists in one template and has no identity outside it. A
**snippet** is the same fix without the ceremony.

## Snippets

Define with `{#snippet}`, render with `{@render}`:

```dmk
<!-- src/dashboard.dmk -->
<section>
  {use crate::deploy_feed::DeployFeed}
  {use crate::model::Status}
  {use crate::service_table::ServiceTable}
  {#snippet tile(class, count, caption)}<div class={class}><div class="n">{count}</div><div class="k">{caption}</div></div>{/snippet}
  <h1>Fleet overview</h1>
  <p class="sub">{self.fleet.services.len()} services · mean uptime {self.fleet.avg_uptime_label()} · worst {self.fleet.worst()}</p>
  <div class="tiles">
    {@render tile("tile healthy", self.fleet.count(Status::Healthy), "healthy")}
    {@render tile("tile degraded", self.fleet.count(Status::Degraded), "degraded")}
    {@render tile("tile down", self.fleet.count(Status::Down), "down")}
    {@render tile("tile", self.fleet.breaching(), "below SLO")}
  </div>
  <ServiceTable services={&self.fleet.services} slo_target={self.fleet.slo_target}/>
  <DeployFeed deploys={&self.fleet.deploys} limit={self.feed_limit}/>
</section>
```

Parameters are what make a snippet more than a copy-paste: the call site decides
what goes in the hole, so the fourth tile has no status behind it at all and
still uses the same markup. Their types are inferred from the calls; where
inference needs help, a parameter takes an annotation —
`{#snippet cell(value: u8)}`.

> [!NOTE]
> Snippets are `let` bindings under the hood, so one must be **defined before it
> is used**. A `{@render}` above its `{#snippet}` will not compile.

```sh
$ cargo run
…
  <div class="tiles">
    <div class="tile healthy"><div class="n">1</div><div class="k">healthy</div></div>
    <div class="tile degraded"><div class="n">1</div><div class="k">degraded</div></div>
    <div class="tile down"><div class="n">0</div><div class="k">down</div></div>
    <div class="tile"><div class="n">1</div><div class="k">below SLO</div></div>
  </div>
…
```

`{@render}` takes anything renderable — a snippet, a slot lookup, a component
value you are holding. What it will not do is build a component from props; that
is what a `<StatusBadge status={…}/>` tag is for.

## The same thing from Rust

A snippet is a closure that writes into a renderer, and `damask::fragment` is how
you make one by hand:

```rust
use damask::{Renderer, fragment};

let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
```

That is renderable content like any other, so it can fill a slot:

```rust
use damask::{Component, DEFAULT_SLOT, Slot, Slots};

let out = page.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &body)]));
```

which is the same call `main` already makes with a `Dashboard` instead of a
closure. It is mostly useful in tests, where you want a shell rendered with known
children so you can assert on what came out around them — the next chapter writes
one.

A name repeated in the slice resolves to the first entry, and `Slots` is `Copy`,
so passing it on costs nothing.

## What a template can ask

A fallback covers content that is missing. It cannot cover the case where the
markup *around* the content should be missing too — an empty `<footer>` is still
a `<footer>`. For that, a template can ask: the caller's fills are in scope as
`slots`, in any `{ … }` tag.

```dmk
<!-- dialog.dmk -->
<div class="dialog">
  <h2>{self.title}</h2>
  {#if slots.has_default()}<p class="body"><slot/></p>{/if}
  {#if slots.has("actions")}<footer>{@render slots.get("actions")}</footer>{/if}
</div>
```

With nothing filled, that renders `<div class="dialog"><h2>…</h2></div>` and
nothing else.

The last line places a fill two ways at once. `slots.get("actions")` is the
content by name, and `{@render}` takes it as it comes — an unfilled slot renders
nothing, the same rule an `Option` attribute follows — so the `{#if}` is guarding
the `<footer>`, not the fill. Where no wrapper is at stake, `<slot/>` says it
shorter.

## Passing slots onward

helm has one shell. The moment it has two — a wide layout and a narrow one, both
inside the same document — the outer one has to hand its caller's content to the
inner one without looking at it. A `<slot>` placed *where a fill goes* does that:

```dmk
<!-- shell.dmk -->
{use crate::frame::Frame}
<Frame title={self.title.clone()}>
  <slot/>                                   <!-- forward the default slot -->
  <slot name="footer" slot="footer"/>       <!-- forward "footer" -->
</Frame>
```

The second line is worth reading twice: `name="footer"` resolves against *this*
component's caller, and `slot="footer"` hands whatever came back to `Frame`. The
first needs no `slot=` — with no name to route it to, a bare `<slot/>` is
ordinary default-slot content, so it forwards on its own and can sit alongside
other markup in the same fill.

Content travels two components deep, still borrowed, still never turned into a
string. [Slots](/docs/slots/) has the resolution rules in full.

The page is now complete. What is left is everything around it: writing the
output somewhere useful, reading it back, and testing that it says what you meant.
