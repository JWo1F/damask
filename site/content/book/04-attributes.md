+++
title = "Attributes and classes"
summary = "How a value decides how it appears, the four forms of `class`, and spreading."
+++

On an HTML element, `attr={expr}` does not stringify the value — it asks the
value's *type* how an attribute should appear. That is the `Attr` trait, and it
is why booleans and options do the right thing without a conditional.

| Value type | Renders |
|---|---|
| `bool` | a bare ` attr` when true, **nothing** when false |
| `Option<T>` | nothing when `None`, otherwise as `T` |
| `&str`, `String`, numbers, `char` | ` attr="escaped"` |

```dmk
<input name="email"
       required={self.required}
       placeholder={self.hint.clone()}
       maxlength={self.limit}>
```

With `required: false`, `hint: None` and `limit: Some(80)`, that renders
`<input name="email" maxlength="80">`.

> [!WARNING]
> Never write `disabled="{self.locked}"`. A quoted value always produces the
> attribute, and in HTML the attribute's *presence* is what disables the control
> — so `disabled="false"` is a disabled control. Use a `bool`.

There is no blanket `Display` impl. A type of your own either implements `Attr`
or reaches the template as a string.

## Quoted values interpolate

```dmk
<tr title="row {self.n} of {self.total}">
```

This is the form to reach for when you are building a string; `{ … }` is the form
to reach for when you are passing a value.

## Class lists

`class` — and only `class` — takes three further forms beyond the quoted one.

```dmk
<div class="card shadow"                                   <!-- quoted, interpolating -->
     class=[self.extra, "base", { "is-open": self.open }]  <!-- a list -->
     class:is-loading={self.busy}>                         <!-- a directive -->
```

A **list** takes strings, `Option`s of strings, and `{ "name": cond }` maps. A
literal `None` is dropped at compile time. Names are deduplicated, keep their
first-mention order, and an empty result omits the attribute entirely.

```dmk
<div class=["card", self.tone.skin(), self.class]>
```

That last entry is a convention worth adopting early: a `class: String` prop lets
a call site add spacing or layout to a component without the component knowing
anything about it.

A **directive**, `class:name={cond}`, adds or removes one name and takes
precedence over whatever the list produced. A bare `class:name` is always on.

> [!CAUTION]
> A directive puts the class name in the *attribute name*, where Tailwind and
> other CSS scanners do not look — the rule gets compiled out of your stylesheet.
> When a class has to be discoverable by a scanner, use the map form, whose names
> are ordinary strings: `class={ "animate-pulse": self.busy }`.

## Spreading

`{...expr}` splices a prepared run of attributes — the ones a component cannot
name, like a computed `data-<controller>-target`:

```dmk
<input {...self.wiring} {...&self.data}>
```

`AttrSpread` is implemented for two things, and the difference between them is a
security boundary:

- **`&'static str`** — markup the author wrote. The `'static` lifetime is what
  keeps a request-derived value out: a string built from a form field cannot be
  `'static`, so it cannot arrive here. Emitted verbatim.
- **`[(K, V)]` and `Vec<(K, V)>`** — a map, escaped on the way out. This is where
  anything derived from state belongs.

```rust
impl Field {
    /// Stimulus wiring this component cannot name. Author-written, so `'static`.
    const WIRING: &'static str = r#"data-controller="autosize" data-action="input->autosize#fit""#;

    /// Values that came from state — escaped, so a hostname containing a quote
    /// stays a hostname.
    fn data(&self) -> Vec<(String, String)> {
        vec![("data-host".into(), self.host.clone())]
    }
}
```

Spreading is only available on HTML elements. A component takes named props.
