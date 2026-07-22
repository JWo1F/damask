+++
title = "Why Damask"
summary = "A component model for server-rendered HTML — named slots, checked props, HTML-aware attributes — and what that trade costs."
+++

Damask is a component model. A component is a struct paired with an HTML
template: the struct's fields are its props, and the template declares the holes
a caller fills. That second half is the part most Rust template engines do not
have.

## The compile-time part is not the argument

Damask compiles templates at build time. A derive macro reads the `.dmk` beside
your struct and turns it into a render method, so nothing parses a template at
runtime, there is no context map to look names up in, and errors arrive at the
right time — a misspelled field is a compile error naming the field, because the
generated code is a struct literal and a field access like any other.

That is worth having, and askama, maud, sailfish, ructe and most of the rest of
the ecosystem already do it. Compile-time codegen is how Damask is built, not why
you would choose it. The reason to choose it is what you can say about a *hole*.

## Named slots

The Rust template engines that offer content projection at all tend to top out
at one anonymous body — askama's `caller()`, or ructe's `Content`, which cannot
even be handed onward to a further template. That is enough to wrap something.
It is not enough for a frame whose title, body and footer are all the caller's
business.

```rust
// frame.rs
use damask::Component;

#[derive(Component)]
pub struct Frame {
    pub title: String,
}
```

```dmk
<!-- frame.dmk -->
<section class="frame">
  <h2>{self.title}</h2>
  <slot/>
  <footer><slot name="footer">© anon</slot></footer>
</section>
```

The caller routes content by name, with the same `slot="…"` attribute a browser
custom element uses:

```dmk
{use crate::frame::Frame}
<Frame title={self.heading.clone()}>
  <p>{self.body}</p>                          <!-- the default slot -->
  <span slot="footer">© {self.year}</span>
  <a slot="footer" href="/about">About</a>
</Frame>
```

Nothing there is a callback, a closure or a pre-rendered string. It is markup, in
the caller's template, landing where the component said it should.

## The fills are borrowed

Slot content stays on the caller's stack. It can borrow the caller's data, and
nothing is boxed or type-erased on the way down — a fill travels as a `&dyn
Render` in an argument to one render call. Reactive frameworks that do offer
named slots pay for them with an allocation and a type erasure; markup-builder
crates pay by rendering the child into a `String` first. Here a slot costs a
pointer.

## Slots are not fields

A template can add or drop a `<slot>` and the struct never changes, which is what
keeps a component's type free of `Render` parameters however much content it
takes. The price is that a slot is matched **by name at render time**: a
misspelled `name` renders the fallback instead of failing the build. That is the
one compile-time check Damask trades away, and it is named again wherever slots
are taught.

## Attributes know they are HTML

Damask parses the markup, so it can tell an attribute from a value:

```dmk
<input disabled={self.disabled} placeholder={self.placeholder}/>
```

A `bool` renders a bare `disabled` or nothing at all — never `disabled="false"`,
which HTML reads as *disabled*. An `Option` renders nothing when it is `None`.
Class lists merge, deduplicate, and omit the attribute when they come out empty.
An engine that substitutes text into a string cannot do any of this, because it
never learns that the thing left of the `=` is an attribute.

## Two files, no configuration

A component is `button.rs` and `button.dmk`, side by side. There is no path
attribute to write, no `templates/` directory, no `build.rs`. The derive finds
the template through `Span::local_file()`, which stabilized in Rust 1.88 — this
was not possible on stable before June 2025.

## What askama does better

If you are weighing the two, be clear about what you would be giving up. Askama
has template inheritance (`{% extends %}` and `{% block %}`), `{% include %}`, a
filter library, `no_std`, templates on enums, integrations with every web
framework, and a user base measured in millions of downloads. It is mature,
well-run, and the default answer for good reasons.

Damask has no inheritance because it has no place to put one: a layout is a
component and a page nests inside it, the same way a badge nests inside a row.
Whether that is the better deal depends on whether your pages are documents that
vary block by block, or interfaces assembled out of parts.

## What it costs

Templates are not data any more. You cannot ship a new one without rebuilding and
you cannot let a user supply one, which rules Damask out for a CMS theme system
and rules it in for the pages of an application you deploy as a binary.

There is also no ambient context — no thread-local, no implicit request object.
Anything a deep component needs travels down as a prop. That is a real cost with
a shape worth designing around, and
[Beyond helm](/book/building-a-page/) is about that shape.

What you get in exchange is that `Frame` above: a struct, a template, and holes
with names.

The rest of the book is one application. Starting from `cargo new`, you build
**helm** — a fleet-status page: a table of services, a feed of deploys, the
chrome around them, and a document shell that takes any of it as content. Each
chapter adds a piece and ends with something you can run, and each feature
arrives at the point the page cannot be finished without it. The next chapter is
a dependency, two files, and one `cargo run`.
