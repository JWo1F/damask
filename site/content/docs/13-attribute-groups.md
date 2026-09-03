+++
title = "Attribute groups"
summary = "`{@attrs(…)}` — one set expanded into a run of `<name>-*` attributes."
section = "Templates"
+++

Where [`{@tokens(…)}`](/docs/token-lists/) assembles one value from parts,
`{@attrs(…)}` goes the other way: **one set expanded into a run of
attributes** — the thing a Rails view does with `data: { … }`.

The prefix is the attribute the helper was written on, so `data` is the common
case rather than a special one:

```dmk
<div data={@attrs(self.wiring)}
     data={@attrs(self.base(), self.extra)}
     data={@attrs(controller: "modal", index: self.i)}
     aria={@attrs(label: self.title)}>
```

Each key becomes `<name>-<key>`, so the third line above writes:

```html
<div data-controller="modal" data-index="4">
```

## Positional entries

A positional entry is anything implementing `AttrSet`:

| Value type | Contributes |
|---|---|
| `[(K, V)]`, `[(K, V); N]`, `Vec<(K, V)>` | each pair, in order |
| `BTreeMap<K, V>` | each pair, in key order |
| `HashMap<K, V>` | each pair, **sorted by key** |
| `Attrs` | the entries it already holds |
| `Option<T: AttrSet>` | those entries when `Some`, nothing when `None` |
| `&T where T: AttrSet` | as `T` |

`K` is anything `AsRef<str>`; `V` is an [`IntoAttrValue`](#values). A `HashMap`
has no order of its own, so the group imposes one — otherwise the same state
would render its attributes in a different order each run, which no snapshot
test could live with.

There is deliberately **no impl for a bare string**. A string carries names but
no values, which is what a token list is made of; a group is made of pairs, and
no reading of `"a b"` as one would be more than a guess.

Entries merge left to right, so `@attrs(base, extra)` means "these, then these
override". A literal `None` is dropped at compile time, for the same reason it
is in `@tokens`: a bare `None` has no type to infer from.

## `key: value` entries

The inline form — the direct equivalent of a Rails hash:

```dmk
<span data={@attrs(index: self.i, open: self.open, "row-note": self.note)}>
```

A key is a bare identifier or a string, and it is the part **after** the prefix.
It is never rewritten: `user_id` writes `data-user_id`, not `data-user-id`.
Rails dasherizes because its keys are symbols; here the key is a name you wrote,
and rewriting it would make `data-user_id` unwritable.

A key that could not be written safely is a **compile error**, not an escaped
name — escaping is a value's defence and cannot be a name's, since a space or an
`=` inside a name simply ends it and begins another attribute. See [Names that
cannot be written](/docs/attributes/#names-that-cannot-be-written).

## Values

The value decides how its attribute appears — and whether it appears at all.
This is [`Attr`](/docs/attributes/#attr) one level down, and it answers the two
questions the same way:

| Value type | Renders |
|---|---|
| `bool` | a bare ` data-key` when `true`, **nothing** when `false` |
| `Option<T>` | nothing when `None`, otherwise as `T` |
| `&str`, `String`, `Cow<'_, str>` | ` data-key="escaped"` |
| `char`, `u8`–`u128`, `usize`, `i8`–`i128`, `isize`, `f32`, `f64` | ` data-key="value"` |

```rust
pub trait IntoAttrValue {
    fn into_attr_value(self) -> AttrValue;
}
```

> [!WARNING]
> A bare `data-open` reaches JavaScript as `el.dataset.open === ""`, which is
> **falsy**. Read presence with `"open" in el.dataset`, or carry the string
> `"true"` when a script wants to test the value.
>
> ARIA wants the string too: `aria-expanded` must read `"true"` or `"false"`, so
> write `aria={@attrs(expanded: self.open.to_string())}` rather than passing the
> `bool`, which would write a bare `aria-expanded` and mean neither.

Values are scalars. A nested map does not flatten into `data-parent-child`, and
nothing is serialised to JSON on your behalf — a type of your own either
implements `IntoAttrValue` or reaches the template as a string.

## Precedence

Everything lands in one `Attrs`, in the order written. A key mentioned twice
keeps the **first position** it was given and takes the **last value**:

```dmk
<div data={@attrs(&self.base, size: "narrow")}>
```

If `base` already had `size`, the attribute stays where `base` put it and reads
`narrow`.

## What a group does *not* touch

**Any attribute that has no helper on it.** `data="movie.swf"` and
`data={self.url}` are the ordinary attribute they look like — which is what
leaves `<object>` working, with no rule to remember about it. It is the helper
that expands, not the name.

**Longhand `data-*` attributes.** They stay on the `Attr` path whether or not a
group sits beside them, so their values are held to `Attr` rather than to
`IntoAttrValue`, and adding a group cannot change how the one next to it
compiles. They are also not deduplicated against the group — two sources naming
the same attribute write it twice.

```dmk
<div data-controller="modal" data={@attrs(index: self.i)}>
```

## `Attrs`

The type the lowered code builds — the same bag a [`#[prop(rest)]`
field](/docs/rest-attributes/) carries, since both are name/value pairs waiting
to be written.

| Item | Purpose |
|---|---|
| `Attrs::new()` | an empty set |
| `.insert(name, value)` | set `name`, whatever `IntoAttrValue` says it is |
| `.insert_bare(name)` | set `name` with no value |
| `.remove(name)` | drop `name`, whatever set it |
| `.get(name)`, `.contains(name)`, `.iter()` | inspect |
| `.merge(&set)` | fold another `AttrSet` in |
| `.write_attrs_prefixed(prefix, r)` | write every entry as `prefix-name` |
| `AttrSet` | what a positional entry implements |

`Attrs` also implements `AttrSpread`, so one built in Rust can be spliced with
`{...expr}` — written out under its own names, with no prefix.

## On a component

`{@attrs(…)}` writes attribute *names*, and a prop is one value under a name the
component chose, so it has nowhere to go on a component tag and is an error
there. A whole set reaches a component either as an ordinary prop:

```rust
#[derive(Component)]
pub struct Modal {
    pub data: Option<Vec<(String, String)>>,
}
```

```dmk
<div data-controller="modal" data={@attrs(self.data.as_deref())}><slot/></div>
```

or, for the attributes the component cannot name, through
[`{...expr}`](/docs/rest-attributes/).
