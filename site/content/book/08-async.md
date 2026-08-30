+++
title = "The live fleet"
summary = "Awaiting inside a template, the pair of traits that swaps in, and rendering a page from a request handler."
+++

helm's fleet is a fixture. `demo_fleet()` builds one in `main.rs`, every
component takes it as a prop, and the page renders in microseconds because
nothing it needs lives anywhere but memory.

A deployed helm has a store behind it — deploys from an API, health from a
database — and those calls are async:

```rust
pub struct FleetStore { /* a pool, a client */ }

impl FleetStore {
    pub async fn recent_deploys(&self, limit: usize) -> Vec<Deploy> { … }
}
```

This chapter is what that does to a template, which for most of a page is
nothing at all.

## Load first, where you can

The best answer is still the one you already have: await in the handler and pass
the result as a prop.

```rust
let deploys = store.recent_deploys(10).await;
let html = Dashboard { fleet: &fleet, deploys: &deploys, feed_limit: 5 }.render();
```

Nothing in this chapter applies to that, and that is the point of mentioning it
first. A component whose props are already loaded renders synchronously, which
is one less thing between a struct and a `String` — and it keeps the loading in
one place you can read top to bottom, which is where you want it the day a page
turns out to be making eleven queries.

Reach for the rest of this chapter when the component itself owns the source: a
feed that decides its own limit, or a widget used on nine pages that would
otherwise make every one of them fetch on its behalf.

## Awaiting in a template

Give `DeployFeed` the store instead of the rows, and ask for them in the markup:

```rust
// src/deploy_feed.rs
use damask::Component;

#[derive(Component)]
pub struct DeployFeed<'a> {
    pub store: &'a FleetStore,
    pub limit: usize,
}
```

```dmk
<!-- src/deploy_feed.dmk -->
<ul class="feed">
  {#for deploy in self.store.recent_deploys(self.limit).await}
    <li>{deploy.service} · {deploy.when()}</li>
  {/for}
</ul>
```

That `.await` is the entire feature. There is no attribute to add, no `async` to
write on the struct, nothing to turn on in `Cargo.toml`: the derive reads the
template, finds an `.await` in the Rust it holds, and compiles an async render
path instead of the sync one.

It looks at the template's **own** Rust — a `{ … }` tag, an `{#if}` condition, a
`{#for}` iterable, an attribute value, a snippet body — and it tokenizes rather
than searching text, so `{ "deployed.await" }` is a string and not a suspension
point. A template with no `.await` in it is compiled exactly as it was in
chapter seven, at no cost.

## What that swapped

`DeployFeed` no longer implements `Component` and `Render`. It implements
`AsyncComponent` and `AsyncRender`, and the method you call has a different
name:

```rust
use damask::AsyncComponent;

let html = DeployFeed { store: &store, limit: 5 }.render_async().await;
```

Call the old one and rustc is unusually helpful about it:

```text
error[E0599]: no method named `render` found for struct `DeployFeed` in the current scope
help: there is a method `render_async` with a similar name
```

The pair mirrors the one from chapter seven exactly. `AsyncRender` is
`render_into_async` and `render_slots_async`; `AsyncComponent` adds
`default_renderer`, `render_async` and `render_with_async`. Every one of them
hands back a boxed future rather than a value, because an `async fn` in a trait
is not object-safe and these traits are object-safe for the same reason `Render`
is. That box is one allocation per render — and the reason the derive only
reaches for this path when a template actually awaits.

## Mixing the two

An async page is not an all-async page. `ServiceTable` and `StatusBadge` await
nothing and never will, and they drop into an awaiting parent unchanged:

```dmk
<section>
  <ServiceTable services={&self.fleet.services} slo_target={self.fleet.slo_target}/>
  <ul class="feed">
    {#for deploy in self.store.recent_deploys(self.limit).await}
      <li>{deploy.service}</li>
    {/for}
  </ul>
</section>
```

Every `Render` gets an `AsyncRender` for free, and that future has nothing to
poll — the sync render already ran to completion by the time it exists. A sync
child inside an async parent therefore costs what it always cost.

The other direction does not work, and cannot:

```text
error[E0277]: the trait bound `DeployFeed<'_>: Render` is not satisfied
help: the trait `Render` is not implemented for `DeployFeed<'_>`
```

reported at the **parent's** derive, naming the child. An async component has no
sync render to fall back on: producing one would mean blocking on a future
inside whatever executor is already driving the caller, which is how a
single-threaded runtime deadlocks. The fix is to make the parent async as well —
usually by lifting the await into it, which is the load-first advice from the
top of this chapter arriving as a compile error.

> [!NOTE]
> That error comes from the trait bound, not from the scan. The derive reads
> only its own template and never looks inside a child, so a `<Card/>` that
> turns out to be async is caught at the call site rather than by the pass that
> decides whether *this* template awaits.

## Rendering from a handler

Which is most of what an async template is for:

```rust
async fn dashboard(State(store): State<Arc<FleetStore>>) -> Html<String> {
    Html(Dashboard { store: &store, limit: 5 }.render_async().await)
}
```

A server executor steals work between threads, so it can only drive a future
that is `Send`. A render future holds exactly three things across each of its
awaits — the renderer it writes through, the slots it was handed, and the
component's own `&self` — and each carries the bound that makes the whole thing
`Send`: `Renderer: Send`, a slot fill is `Sync`, and `AsyncRender: Sync`.

You will not normally meet any of them. A renderer is a buffer and a component
is a struct of data, so both are `Send`/`Sync` on their own. Two places it does
surface: a **generic** component that awaits has to name `Sync` on the
parameters it holds, and a hand-written `Renderer` over a deliberately
non-`Send` sink can no longer be used at all — with a sync template or an async
one.

## Two places `.await` can't go

Both are narrow, both have a one-line rewrite, and in both cases the compiler
tells you what it is.

**A component's slot fill.** The children between `<Comp>…</Comp>` reach the
callee as a plain `&dyn Render`, and there is no async render behind that
reference:

```text
error: `.await` is not supported inside `<Frame>`'s default slot content;
       compute the value in a `{ let x = … }` above this tag and pass `{x}` instead
```

So compute it first and pass the result in:

```dmk
{ let latest = self.store.latest_deploy().await }
<Frame title="Last deploy"><p>{latest.service}</p></Frame>
```

A `<slot>`'s own **fallback** body — back in the component that declares the
slot — has no such limit. It is ordinary markup in an async template and awaits
like the rest of it.

**A `{#snippet}` that takes parameters.** A parameterless snippet is a
`Fragment` and awaits freely; one with parameters is a closure that must stay
callable more than once, which an `async move` capturing a non-`Copy` parameter
cannot promise:

```text
error: `{#snippet row(…)}` cannot both take parameters and `.await` in its own body;
       move the `.await` to where it's rendered instead
```

Which is the same move as before — await at the call site, and let the body use
the value that arrives:

```dmk
{#snippet row(deploy: Deploy)}<li>{deploy.service}</li>{/snippet}
<ul class="feed">
  {#for deploy in self.store.recent_deploys(self.limit).await}
    {@render row(deploy)}
  {/for}
</ul>
```

## Testing one

A test that awaits needs something to await in. Where a runtime is already in
the dependency tree, that is its test attribute:

```rust
#[tokio::test]
async fn the_feed_lists_what_the_store_returns() {
    let store = FleetStore::fake(&[("api", "9f3c1ab")]);
    let out = DeployFeed {
        store: &store,
        limit: 5,
    }
    .render_async()
    .await;

    assert!(out.contains("9f3c1ab"), "{out}");
}
```

Damask has no runtime dependency of its own, and its async tests do not add one:
nothing in a render suspends unless *your* code does, so a component whose store
is a fixture resolves on the first poll. `examples/showcase/tests/async_render.rs`
in the repository polls such a future in a loop with a no-op waker and skips the
runtime entirely — worth copying if you want an async component under test
without pulling a runtime into a crate that has none.

helm can load now. The last chapter is about the second application — the
conventions that keep a directory of these honest once there are forty of them.
