+++
title = "Attributes"
summary = "The `Attr` trait, quoted interpolation, and spreading."
section = "Templates"
+++

## Forms

| Form | Meaning |
|---|---|
| `attr="text"` | literal, interpolating `{ … }` tags inside it |
| `attr='text'` | the same, single-quoted |
| `attr={expr}` | the value's type decides how it appears (`Attr`) |
| `attr` | a bare boolean attribute, always present |
| `{...expr}` | splice a prepared run of attributes (`AttrSpread`) |

An attribute name is made of letters, digits, `_`, `-` and `:`, so
`data-controller`, `aria-label` and `xlink:href` all pass through as written.
Attributes reach the output in the order they were written.

## `Attr`

On an HTML element, `attr={expr}` asks the value's type how to render. The name
and the quotes are the value's to write, which is what lets it decline to appear
at all.

```rust
pub trait Attr {
    fn write_attr(&self, name: &str, r: &mut dyn Renderer);
}
```

| Value type | Renders |
|---|---|
| `bool` | a bare ` attr` when `true`, **nothing** when `false` |
| `Option<T: Attr>` | nothing when `None`, otherwise as `T` |
| `&str`, `String`, `Cow<'_, str>` | ` attr="escaped"` |
| `char`, `u8`–`u128`, `usize`, `i8`–`i128`, `isize`, `f32`, `f64` | ` attr="value"` |
| `&T` where `T: Attr` | as `T` |

```dmk
<input name="email" required={self.required} maxlength={self.limit}>
```

There is deliberately no blanket `Display` impl: it would collide with the `bool`
and `Option` impls, which are the point. A type of your own either implements
`Attr` or reaches the template as a string.

> [!WARNING]
> Use a `bool` for HTML boolean attributes. `disabled="{self.locked}"` always
> emits the attribute, and in HTML the attribute's presence is what disables the
> control — so `disabled="false"` is a disabled control.

## Interpolation

A quoted value interpolates, and the interpolated parts are escaped by the same
policy as `{ … }` in text. There are no backslash escapes inside a quoted value;
the closing quote ends it, except inside a `{ … }` hole.

```dmk
<tr title="row {self.n} of {self.total}" id="row-{self.id}">
```

A value with no holes stays literal text in the compiled output. One with holes
becomes a `format!`.

On a component prop, a quoted value also **converts** — it reaches an
`Option<String>` prop as `Some(…)` without `Some` at the call site. A `{ … }`
value stays exactly its type. See [Props](/docs/props/).

## Spreading

`{...expr}` splices attributes whose names the template cannot write — a computed
`data-<controller>-target`, or a map built in Rust.

```dmk
<input {...self.wiring} {...&self.data}>
```

`AttrSpread` is implemented for:

`&'static str`
: Markup the author wrote, emitted verbatim after a single space, and skipped
  when empty. The `'static` bound is the guarantee: a string derived from a
  request or a config field cannot be `'static`, so it cannot arrive here.

`[(K, V)]` and `Vec<(K, V)>` where `K, V: AsRef<str>`
: A map. The value is **escaped** on the way out, and the name is **checked**.
  This is where anything derived from state belongs.

`Option<T>` and `&T` where `T: AttrSpread`
: `None` writes nothing; a reference writes what it points at.

`DataSet`
: The set the [`data`](/docs/data-attributes/) forms build, so one assembled in
  Rust can be spliced rather than passed as a prop.

```rust
const WIRING: &'static str = r#"data-controller="confirm" data-action="confirm#check""#;

fn data(&self) -> Vec<(String, String)> {
    vec![("data-host".into(), self.host.clone())]
}
```

Spreading is available on HTML elements only — a component takes named props, and
a spread carries no name to give one. `{...}` with an empty expression is an
error, as is `{...}` on a component tag.

## Optional runs

An `Option` of a spreadable value is the idiom for a group of attributes that are
all present or all absent together:

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

## Names that cannot be written

Escaping is a *value's* defence and cannot be a *name's*. A space or an `=`
inside an attribute name does not need escaping to be dangerous — it simply ends
the name and begins a second attribute, so a key like `x onclick` would smuggle
one in whole. Names that could do that are therefore **refused rather than
escaped**: control characters, whitespace, `" ' > / =`, and `< &`.

```rust
pub fn is_attr_name_safe(name: &str) -> bool;
```

The key/value `AttrSpread` and every [`data`](/docs/data-attributes/) key are
held to this. A name that fails it is dropped, and trips a `debug_assert` — loud
in a debug build, where it can be fixed, and harmless in a release one, where it
cannot. Names you write in the template are unaffected: the parser would not have
accepted one.

## `class` and `data` are the exceptions

`class` accepts three further forms of its own, and a `class:name` directive that
overrules them. See [Class lists](/docs/class-lists/).

`data` accepts an expression, a list and a map, and expands into a run of
`data-*` attributes. See [Data attributes](/docs/data-attributes/).

Any other attribute given a class list or a data map is an error.
