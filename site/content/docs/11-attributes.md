+++
title = "Attributes"
summary = "The `Attr` trait, quoted interpolation, and spreading."
section = "Templates"
+++

## Forms

| Form | Meaning |
|---|---|
| `attr="text"` | literal, interpolating `{ … }` tags inside it |
| `attr={expr}` | the value's type decides how it appears (`Attr`) |
| `attr` | a bare boolean attribute, always present |
| `{...expr}` | splice a prepared run of attributes (`AttrSpread`) |

## `Attr`

On an HTML element, `attr={expr}` asks the value's type how to render.

| Value type | Renders |
|---|---|
| `bool` | a bare ` attr` when `true`, **nothing** when `false` |
| `Option<T>` | nothing when `None`, otherwise as `T` |
| `&str`, `String` | ` attr="escaped"` |
| integers, floats, `char` | ` attr="value"` |

```dmk
<input name="email" required={self.required} maxlength={self.limit}>
```

There is no blanket `Display` impl. A type of your own either implements `Attr`
or reaches the template as a string.

> [!WARNING]
> Use a `bool` for HTML boolean attributes. `disabled="{self.locked}"` always
> emits the attribute, and in HTML the attribute's presence is what disables the
> control — so `disabled="false"` is a disabled control.

## Interpolation

A quoted value interpolates:

```dmk
<tr title="row {self.n} of {self.total}" id="row-{self.id}">
```

On a component prop, a quoted value also **converts** — it reaches an
`Option<String>` prop as `Some(…)` without `Some` at the call site. A `{ … }`
value stays exactly its type.

## Spreading

`{...expr}` splices attributes whose names the template cannot write — a computed
`data-<controller>-target`, or a map built in Rust.

```dmk
<input {...self.wiring} {...&self.data}>
```

`AttrSpread` is implemented for:

`&'static str`
: Markup the author wrote, emitted verbatim. The `'static` bound is the
  guarantee: a string derived from a request or a config field cannot be
  `'static`, so it cannot arrive here.

`[(K, V)]` and `Vec<(K, V)>`
: A map, **escaped** on the way out. This is where anything derived from state
  belongs.

```rust
const WIRING: &'static str = r#"data-controller="confirm" data-action="confirm#check""#;

fn data(&self) -> Vec<(String, String)> {
    vec![("data-host".into(), self.host.clone())]
}
```

Spreading is available on HTML elements only. Components take named props.

## Optional runs

An `Option` of a spreadable value works, which is the idiom for a group of
attributes that are all present or all absent together:

```rust
/// Three attributes that must never disagree, hence one accessor.
fn confirm_wiring(&self) -> Option<[(&'static str, &str); 3]> {
    self.confirm.as_ref().map(|message| {
        [
            ("data-controller", "confirm"),
            ("data-confirm-message-value", message.as_str()),
            ("data-action", "confirm#check"),
        ]
    })
}
```

```dmk
<button {...self.confirm_wiring().as_ref().map(|w| &w[..])}>
```
