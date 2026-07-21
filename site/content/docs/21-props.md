+++
title = "Props"
summary = "Fields are props: passing, skipping, moving, and inference."
section = "Components"
+++

A component's struct fields **are** its props. A component tag is built from its
attributes and becomes the struct literal, so the same rules apply.

```rust
#[derive(Component)]
pub struct Notice {
    pub title: String,
    pub detail: Option<String>,
    pub dismissible: Option<bool>,
}
```

```dmk
<Notice title="Deploy finished"/>
<Notice title="Rollback" detail="check the log" dismissible/>
```

## Skippable props

A prop must be passed unless its **type** says what leaving it out means, and
only `Option<_>` does. A skipped prop arrives as `None`.

> [!NOTE]
> `bool` is required like anything else. A flag a caller may omit is
> `Option<bool>`.

`#[component(default)]` on the struct makes every prop skippable, filling from
`Default` — see [the derive](/docs/derive/#componentdefault).

## Quoted values convert; braced values do not

| At the call site | Reaches an `Option<String>` prop as |
|---|---|
| `detail="check the log"` | `Some("check the log".into())` |
| `detail="row {self.n}"` | `Some("row 3".into())` |
| `detail={self.detail.clone()}` | exactly that `Option` |
| `detail={Some(x)}` | `Some(x)` |

A `{ … }` value **is** the prop's type, including its `Option`. That exactness is
what makes inference work: `count={2 + 8}` infers to whatever integer type the
prop is, and `items={&self.rows}` coerces a `&Vec<T>` to a `&[T]` prop.

## Values move

Attribute values move into the component's fields, so a `&self` field is passed
by clone — or is `Copy`:

```dmk
<Card title={self.heading.clone()} tone={self.tone}/>
```

This is ordinary Rust ownership, not a template rule. If cloning a large value
per render matters, make the prop a borrow and give the component a lifetime;
the derive leaves generics alone.

## Grouping related props

Props that are not independent are worth one type rather than five fields. A
button's `type`, `name`, `value`, `disabled` and `confirm` all describe the same
thing, and most call sites set none of them:

```rust
#[derive(Debug, Clone, Default)]
pub struct Act { /* … */ }

impl Act {
    pub fn inert() -> Self { … }
    pub fn submit() -> Self { … }
    pub fn confirm(mut self, message: impl Into<String>) -> Self { … }
}

#[derive(Component)]
pub struct Button {
    pub act: Act,
    pub class: String,
}
```

```dmk
<Button act={Act::submit().confirm("Reboot the router?")} class="">Reboot</Button>
```

The shape of the call site then says what the button is for, and no template ever
spreads a `Default`.

## No ambient context

There is no request object, thread-local or implicit scope. Everything a
component needs arrives as a prop. In a real application this pushes you toward a
single "shell" value assembled once and passed down — which has the useful
property that what a page depends on is visible in its type.
