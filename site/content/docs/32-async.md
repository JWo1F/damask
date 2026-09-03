+++
title = "Async templates"
summary = "What `.await` in a template changes, the traits it swaps in, and the two places it cannot go."
section = "Runtime"
+++

```dmk
<p>{self.load_name().await}</p>
```

A template containing `.await` compiles to an async render path. There is no
attribute to add and nothing to enable: the derive decides from the template
itself, and a template with no `.await` is compiled exactly as before, at no
extra cost.

Such a component implements `AsyncComponent` / `AsyncRender` **instead of**
`Component` / `Render`, and renders with `.render_async().await`.

```rust
use damask::AsyncComponent;

let html = Profile { user_id }.render_async().await;
```

## What counts as awaiting

The scan walks the parsed template and tokenizes each Rust fragment, so `.await`
is recognised as the token and not as text.

| Place | Counts |
|---|---|
| `{ expr.await }`, `{@html …}`, `{@render …}` | yes |
| `{#if cond.await}` / `{:else if …}` | yes |
| `{#for pat in iter.await}` | yes |
| `attr={expr.await}`, `attr="text {expr.await}"` | yes |
| `attr={@tokens(…)}`, `attr={@attrs(…)}`, `{...spread}` | yes |
| a `{#snippet}` body, and markup nested at any depth | yes |
| `{ "user.await" }` — inside a string literal | **no** |
| a child component that is itself async | **no** — see below |

A fragment that fails to tokenize is treated as await-free; lowering reaches the
same text and reports the real parse error.

**Nested components are not scanned.** The derive sees only its own template, so
an async child in a sync parent is not caught here — it surfaces at the call
site as a missing `Render` impl, reported at the parent's derive:

```text
error[E0277]: the trait bound `AsyncGreeting: Render` is not satisfied
help: the trait `Render` is not implemented for `AsyncGreeting`
```

## The traits

```rust
pub type RenderFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

pub trait AsyncRender: Sync {
    fn render_into_async<'a>(&'a self, r: &'a mut dyn Renderer) -> RenderFuture<'a>;
    fn render_slots_async<'a>(&'a self, r: &'a mut dyn Renderer, slots: Slots<'a>)
        -> RenderFuture<'a> { … }
}

pub trait AsyncComponent: AsyncRender {
    fn default_renderer(&self) -> Box<dyn Renderer>;
    fn render_async(&self) -> Pin<Box<dyn Future<Output = String> + Send + '_>> { … }
    fn render_with_async(&self, slots: Slots<'_>)
        -> Pin<Box<dyn Future<Output = String> + Send + '_>> { … }
}
```

*(Lifetimes elided here for legibility; the real signatures name them the way
`async-trait` does.)*

Method for method, this is [`Render` and `Component`](/docs/traits/) with the
values replaced by futures, and the shape is the same one: the derive implements
`render_slots_async` with the lowered template and points `render_into_async` at
it with `Slots::EMPTY`; `render_async` is `render_with_async(Slots::EMPTY)`; and
`render_with_async` builds the default renderer, awaits `render_slots_async`,
and returns `finish()`. `render_slots_async`'s own default forwards to
`render_into_async`, so hand-written content with no slots stays a one-method
impl.

**The futures are boxed** because an `async fn` in a trait is not `dyn`-safe,
and these traits are object-safe for the same reason `Renderer` and `Render`
are. That is one allocation per render, which is exactly what `Render` avoids —
so the derive emits this path only for a template that actually awaits.

## Composition

| Parent | Child | Works |
|---|---|---|
| async | sync | **yes**, via the blanket impl — no real cost |
| async | async | yes |
| sync | sync | yes |
| sync | async | **no** — compile error at the parent's derive |

```rust
impl<T: Render + Sync + ?Sized> AsyncRender for T { … }
impl<T: Component + Sync + ?Sized> AsyncComponent for T { … }
```

Every sync `Render` is therefore an `AsyncRender` whose future has nothing to
poll: the sync render already ran to completion by the time the future exists.
The same impls are why `render_async` and `render_with_async` work on a sync
component too.

The reverse has no impl because there is no sensible one. Rendering an async
component synchronously would mean blocking on a future inside whatever executor
is already driving the caller. Either make the enclosing template async as well,
or make the child sync by loading its data before it is constructed and passing
that in as a prop.

## Why the `Send` and `Sync` bounds

An async render is driven by the caller's executor, and a work-stealing executor
cannot drive a future that is not `Send` — without these, an async template
could not be rendered from a request handler at all, which is most of what the
feature is for. Each bound covers one thing the future holds across an `.await`:

| Bound | Covers | In practice |
|---|---|---|
| `Renderer: Send` | the `&mut dyn Renderer` written through | a buffer is `Send` unless deliberately built otherwise |
| `Slot`'s content is `&(dyn Render + Sync)` | the `Slots` carried past each await | on `Slot`, **not** a `Render` supertrait — a component that neither fills a slot nor awaits is unaffected |
| `AsyncRender: Sync` | the component's own `&self` | automatic for a struct of data; a **generic** component that awaits must name `Sync` on the parameters it holds |

## Slots

A `<slot>` in an async template lowers to `Slots::render_async`, whose fallback
hands back a future instead of running inline:

```rust
pub fn render_async<'r>(
    &self,
    name: &str,
    r: &'r mut dyn Renderer,
    indent: usize,
    fallback: impl FnOnce(&'r mut dyn Renderer) -> RenderFuture<'r>,
) -> RenderFuture<'r>
```

A slot's **fallback body** may await freely. A caller's **fill** may not — see
below. Fills reach an async template through `render_with_async`, exactly as
they reach a sync one through `render_with`.

## Snippets and fragments

```rust
pub struct AsyncFragment<F>(pub F);
pub fn fragment_async<F>(f: F) -> AsyncFragment<F>
where
    F: for<'r> Fn(&'r mut dyn Renderer) -> RenderFuture<'r>;
```

The async counterpart of [`fragment`](/docs/snippets/#fragments): what a
`{#snippet}` lowers to when its enclosing template awaits. A parameterless
snippet becomes an `AsyncFragment` and may await; one with parameters may not.

## The two places `.await` cannot go

Both are compile errors that name their own rewrite.

### A component's slot fill

The children between `<Comp>…</Comp>`, or a `slot="x"` child, travel to the
callee as a plain `&dyn Render` — there is no async render behind that
reference.

```text
error: `.await` is not supported inside `<Frame>`'s default slot content;
       compute the value in a `{ let x = … }` above this tag and pass `{x}` instead
```

```dmk
{ let latest = self.store.latest().await }
<Frame title="Last deploy"><p>{latest.service}</p></Frame>
```

### A parameterized `{#snippet}`

A snippet with parameters lowers to a closure that must stay callable more than
once, which an `async move` capturing a non-`Copy` parameter cannot guarantee.

```text
error: `{#snippet row(…)}` cannot both take parameters and `.await` in its own body;
       move the `.await` to where it's rendered instead
```

```dmk
{#snippet row(deploy: Deploy)}<li>{deploy.service}</li>{/snippet}
{#for deploy in self.store.recent(self.limit).await}{@render row(deploy)}{/for}
```

## Asking whether a template awaits

```rust
pub fn template_awaits(
    source_file: Option<&Path>,
    name_snake: &str,
    explicit: Option<&str>,
) -> bool
```

For a framework that wraps `#[derive(Component)]` and emits code of its own
beside the derive's: what a component *is* differs between the two cases — an
awaiting one implements `AsyncComponent` and has no `Component` for a response
impl to hang on — and a wrapper macro has no type information to tell them
apart, while rustc rejects an impl whose `where Self: Component` is trivially
false. So the question is answered from the struct instead.

The arguments are the derive's own template resolution: the file the struct was
written in, its name in snake_case, and an explicit `#[template(path = "…")]` if
there is one. A template that cannot be found or parsed answers `false` — the
derive is about to report that failure properly, and a second copy would bury
it.

`damask_template::needs_async(&template)` is the same question asked of an
already-parsed template.

## Testing

Damask has no runtime dependency, and testing an async component does not
require adding one: nothing in a render suspends unless the awaited code does,
so a component over a fixture resolves on its first poll. `#[tokio::test]` works
where a runtime is already present; `examples/showcase/tests/async_render.rs`
shows the runtime-free alternative.
