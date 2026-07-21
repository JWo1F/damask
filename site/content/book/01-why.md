+++
title = "Why compile-time components"
summary = "What Damask does differently, and the trade that buys."
+++

Most template engines are interpreters. You hand them a string at startup, they
parse it into a tree, and on every request they walk that tree, look each name up
in a context map, and append to a buffer. The template is data, and your program
is a machine for reading it.

Damask moves all of that to build time. A template is compiled — by a derive
macro, into a `render` method on your struct — and what survives to runtime is
straight-line Rust.

## What that changes

**Errors arrive at the right time.** A misspelled field is a compile error naming
the field, not a blank space on a page in production. A component called with the
wrong props fails the build for the same reason a struct literal with a missing
field does — because that is what it becomes.

**There is no context map.** A `{ … }` tag holds a Rust block, evaluated in the
scope of the `render` method. It sees `self`, it sees methods on the `impl` next
door, it sees anything you brought in with `{use}`. Nothing has to be registered,
converted into a dynamic value type, or looked up by string at render time.

**Rendering is cheap and boring.** The output is a run of `write_raw` calls over
string literals the compiler already knows the length of, with your values
escaped into the gaps. There is no template cache to warm, invalidate, or get
wrong.

## What it costs

Templates are not data any more. You cannot ship a new template without
rebuilding, and you cannot let a user supply one — which rules Damask out for a
CMS theme system and rules it in for the pages of an application you deploy as a
binary.

The other cost is honest to name: slots are matched by name at render time, not
at compile time. A misspelled `<slot name="…">` renders its fallback instead of
failing the build. That is the price of keeping slots off the struct, and it is
discussed where slots are.

## What it looks like

A component is two files that share a basename:

```rust
// greeting.rs
use damask::Component;

#[derive(Component)]
pub struct Greeting {
    pub name: String,
}
```

```dmk
<!-- greeting.dmk -->
<p>Hello {self.name}!</p>
```

```rust
assert_eq!(
    Greeting { name: "Ada".into() }.render(),
    "<p>Hello Ada!</p>",
);
```

And because `{ … }` escapes, the interesting case is handled without you
thinking about it:

```rust
assert_eq!(
    Greeting { name: "<b>".into() }.render(),
    "<p>Hello &lt;b&gt;!</p>",
);
```

That is the whole idea. The rest of the book is the details.
