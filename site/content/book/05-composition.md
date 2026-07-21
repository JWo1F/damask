+++
title = "Composition and slots"
summary = "Calling one component from another, passing children, and forwarding them on."
+++

**A capitalised tag is a component.** Lowercase tags are HTML; anything starting
with an uppercase letter is built from its attributes and rendered.

```dmk
{use crate::ui::Badge}
<Badge tone={self.tone} label="Online"/>
```

Attributes become fields, so the tag is checked like the struct literal it is: a
missing required field is a compile error naming it.

Values **move** into the component's fields, so a `&self` field is passed by
clone:

```dmk
<Card title={self.heading.clone()} tone={self.tone}/>
```

`Tone` there is `Copy`, so it needs nothing. This is ordinary Rust ownership, not
a template rule.

## Skippable props

A prop must be passed unless its **type** says what leaving it out means — and
only `Option<_>` does. A skipped one arrives as `None`:

```rust
#[derive(Component)]
pub struct Notice {
    pub title: String,             // required
    pub detail: Option<String>,    // skippable
    pub dismissible: Option<bool>, // a flag a caller may omit
}
```

```dmk
<Notice title="Deploy finished"/>
<Notice title="Rollback" detail="check the log" dismissible/>
```

A **quoted** value converts into the `Option` for you — `detail="check the log"`
arrives as `Some`. A `{ … }` value stays exact, so forwarding one is
`detail={self.detail.clone()}` and constructing one is `detail={Some(x)}`. That
exactness is what lets `count={2 + 8}` infer to the prop's integer type.

> [!NOTE]
> `bool` is required like any other type. A flag a caller may leave out is
> `Option<bool>`, not `bool`.

If a struct's defaults are meaningful rather than zero values, `#[component(default)]`
makes every prop skippable and fills the gaps from `Default`:

```rust
#[derive(Component)]
#[component(default)]
pub struct Theme {
    pub accent: String,
    pub label: String,
    pub dense: bool,
}
```

```dmk
<Theme/>                   <!-- every prop from Default -->
<Theme label="Compact"/>   <!-- accent and dense from Default -->
```

## Slots

A component places caller-supplied content with `<slot/>`.

```rust
#[derive(Component)]
pub struct Frame {
    pub title: String,
}
```

```dmk
<!-- frame.dmk -->
<section>
  <h2>{self.title}</h2>
  <slot/>
  <footer><slot name="footer">© anon</slot></footer>
</section>
```

**Slots are not fields.** A template declares as many as it likes and the struct
never changes — which is exactly why they cannot be checked at compile time.

A `<slot>`'s body is its **fallback**, rendered when the caller leaves it
unfilled. Filling it from a call site looks like this:

```dmk
{use crate::ui::Frame}
<Frame title={self.heading.clone()}>
  <p>{self.body}</p>                  <!-- fills the default slot -->
  <slot name="footer">© {self.year}</slot>
</Frame>
```

Content that is not inside a named `<slot>` fills the default one.

> [!IMPORTANT]
> Slots are matched **by name at render time**. A misspelled `name` renders the
> fallback instead of failing the build, and filling a slot the template does not
> declare is silently ignored. This is the one place Damask trades a compile-time
> check away, and it buys slots that do not appear on the struct.

## Forwarding

A `<slot>` placed directly inside a component element fills that component's slot
of the same name. A **bare** `<slot/>` there is still a placeholder — so it
forwards the caller's content straight through:

```dmk
<!-- shell.dmk -->
{use crate::ui::Frame}
<Frame title={self.title.clone()}>
  <slot/>                                             <!-- forward the default slot -->
  <slot name="footer"><slot name="footer"/></slot>    <!-- fill, wrapping a placeholder -->
</Frame>
```

The second line is the pattern worth reading twice: the outer `<slot name="footer">`
*fills* `Frame`'s footer, and the inner one is a placeholder for whatever this
component's own caller passed as `footer`.

## Layouts

Put those together and a layout is just a component that is almost entirely
slots:

```dmk
<!-- base.dmk -->
<!DOCTYPE html>
<html lang="en">
<head>
  <title>{self.title}</title>
  <link rel="stylesheet" href="/assets/app.css">
</head>
<body><slot/></body>
</html>
```

```dmk
<!-- page.dmk -->
{use crate::layouts::Base}
<Base title={self.title.clone()}>
  <h1>{self.heading}</h1>
  <slot/>
</Base>
```

There is no layout mechanism to learn, because there is no layout mechanism —
a document is a component, and a page nests inside it the same way a badge nests
inside a row.
