+++
title = "Props"
summary = "Fields are props: passing, skipping, moving, and inference."
section = "Components"
+++

A component's struct fields **are** its props. A component tag names the props
the author wrote and is compiled into a builder that constructs the struct, so
ordinary Rust rules apply to the values.

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

The check is syntactic, on the last segment of the path — `std::option::Option<T>`
counts, a type alias for it does not.

> [!NOTE]
> `bool` is required like anything else. A flag a caller may omit is
> `Option<bool>`. A bare `dismissible` at the call site is passed as
> `true.into()`, so it reaches either.

`#[component(default)]` on the struct makes every prop skippable, filling from
`Default` — see [the derive](/docs/derive/#componentdefault).

Forgetting a required prop is a trait-bound error at the call site that names it:

```text
missing a required prop: __DamaskNotice_title
```

## Quoted values convert; braced values do not

| At the call site | Reaches an `Option<String>` prop as |
|---|---|
| `detail="check the log"` | `Some("check the log".into())` |
| `detail="row {self.n}"` | `Some("row 3".into())` |
| `detail={self.detail.clone()}` | exactly that `Option` |
| `detail={Some(x)}` | `Some(x)` |

A `{ … }` value **is** the prop's type, including its `Option`. That exactness is
what makes inference work: a setter takes the prop's type exactly, as assigning
to the field did, so `count={2 + 8}` infers to whatever integer type the prop is
and `items={&self.rows}` coerces a `&Vec<T>` to a `&[T]` prop.

The conversion therefore happens on the *value* side instead. An interpolated
value is already a `String` and reaches both a `String` and an `Option<String>`
prop through `Into`. Static text is the case `Into` cannot serve — no
`From<&'static str> for Option<String>` exists — so `damask::props::literal`
stands in, inferring which conversion it needs from the prop:

| Prop type | What a quoted value needs |
|---|---|
| `T` | `T: From<&'static str>` |
| `Option<T>` | `T: From<&'static str> + From<String>` |

`String`, `Cow<'static, str>`, `Box<str>`, `Rc<str>`, `Arc<str>` and any type of
your own with both conversions qualify. `Option<&'static str>` works too, through
the first row. A prop a quoted value cannot reach reports so:

```text
a quoted attribute value cannot become `…`
```

## Values move

Attribute values move into the component's fields, so a `&self` field is passed
by clone — or is `Copy`:

```dmk
<Card title={self.heading.clone()} tone={self.tone}/>
```

This is ordinary Rust ownership, not a template rule. If cloning a large value
per render matters, make the prop a borrow and give the component a lifetime; the
derive leaves generics alone, and the builder carries them:

```rust
#[derive(Component)]
pub struct Tagged<'a, T: Display> {
    pub value: T,
    pub note: Option<&'a str>,
}
```

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

## What props are not

Slots are not props — a template adds or drops a `<slot>` without the struct
changing. See [Slots](/docs/slots/).

There is also no ambient context: no request object, thread-local or implicit
scope. Everything a component needs arrives as a prop. In a real application this
pushes you toward a single "shell" value assembled once and passed down — which
has the useful property that what a page depends on is visible in its type.

## The `props` module

`damask::props` holds the typestate the builder is made of — `Set`, `Provided`,
`FromText`, `FromLiteral`, `literal`. It exists to phrase the two diagnostics
above. Nothing in it is meant to be named by hand.
