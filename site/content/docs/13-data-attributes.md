+++
title = "Data attributes"
summary = "The forms `data` accepts, and what one value expands into."
section = "Templates"
+++

`data` is the second attribute with forms of its own. Where `class` assembles
one string from parts, `data` expands **one value into a run of attributes** —
the thing a Rails view does with `data: { … }`.

```dmk
<div data={self.wiring}                                    <!-- any DataItem -->
     data=[self.base(), self.extra]                        <!-- list -->
     data={ "controller": "modal", "index": self.i }>      <!-- map -->
```

Each key becomes `data-<key>`, so the map above writes:

```html
<div data-controller="modal" data-index="4">
```

## Expression

`data={expr}` takes anything implementing `DataItem`:

| Value type | Contributes |
|---|---|
| `[(K, V)]`, `[(K, V); N]`, `Vec<(K, V)>` | each pair, in order |
| `BTreeMap<K, V>` | each pair, in key order |
| `HashMap<K, V>` | each pair, **sorted by key** |
| `DataSet` | the entries it already holds |
| `Option<T: DataItem>` | those entries when `Some`, nothing when `None` |
| `&T where T: DataItem` | as `T` |

`K` is anything `AsRef<str>`; `V` is a [`DataValue`](#datavalue). A `HashMap`
has no order of its own, so the set imposes one — otherwise the same state would
render its attributes in a different order each run, which no snapshot test
could live with.

There is deliberately **no impl for a bare string**. A string carries names but
no values, which is what a class list is made of; a data set is made of pairs,
and no reading of `"a b"` as one would be more than a guess.

## List

`data=[…]` merges several sources left to right, and composes with the map form
exactly as a class list does:

```dmk
<div data=[&self.base, self.extra.as_deref(), { "open": self.open }]>
```

A literal `None` is dropped at compile time, for the same reason it is in a
class list: a bare `None` has no type to infer from.

## Map

`data={ "key": value }` is the inline form — the direct equivalent of a Rails
hash. It is told from an ordinary `data={expr}` by a top-level `:` that is not
part of a `::` path, the same test [class maps](/docs/class-lists/#map-alone)
use.

```dmk
<span data={ "index": self.i, "open": self.open, "note": self.note }>
```

A map may also sit inside a list, which is how the two compose.

## `DataValue`

The value decides how its attribute appears — and whether it appears at all.
This is [`Attr`](/docs/attributes/#attr) one level down, and it answers the two
questions the same way:

| Value type | Renders |
|---|---|
| `bool` | a bare ` data-key` when `true`, **nothing** when `false` |
| `Option<T: DataValue>` | nothing when `None`, otherwise as `T` |
| `&str`, `String`, `Cow<'_, str>` | ` data-key="escaped"` |
| `char`, `u8`–`u128`, `usize`, `i8`–`i128`, `isize`, `f32`, `f64` | ` data-key="value"` |
| `&T where T: DataValue` | as `T` |

```rust
pub trait DataValue {
    fn add_to(&self, key: &str, set: &mut DataSet);
}
```

> [!WARNING]
> A bare `data-open` reaches JavaScript as `el.dataset.open === ""`, which is
> **falsy**. Read presence with `"open" in el.dataset`, or carry the string
> `"true"` when a script wants to test the value.

Values are scalars. A nested map does not flatten into `data-parent-child`, and
nothing is serialised to JSON on your behalf — a type of your own either
implements `DataValue` or reaches the template as a string.

## Keys

A key is the part **after** `data-`, and it is never rewritten: `"user_id"`
writes `data-user_id`, not `data-user-id`. Rails dasherizes because its keys are
symbols; here the key is a string you wrote, and rewriting it would make
`data-user_id` unwritable.

A key that could not be written safely is **dropped** rather than escaped — see
[Names that cannot be written](/docs/attributes/#names-that-cannot-be-written).

## Precedence

Everything lands in one `DataSet`, in the order written. A key mentioned twice
keeps the **first position** it was given and takes the **last value**:

```dmk
<div data=[&self.base, { "size": "narrow" }]>
```

If `base` already had `size`, the attribute stays where `base` put it and reads
`narrow`. That is what makes a list mean "these, then these override".

## What `data` does *not* touch

**A quoted value.** `data="…"` is the ordinary attribute it has always been,
which is what leaves `<object data="movie.swf">` alone. A dynamic one there is
written `data="{self.url}"` — the interpolating form, not `data={self.url}`,
which is a data set.

**Longhand `data-*` attributes.** They stay on the `Attr` path whether or not a
`data` value sits beside them, so their values are held to `Attr` rather than to
`DataValue`, and adding a `data` map cannot change how the one next to it
compiles. They are also not deduplicated against the set — two sources naming
the same attribute write it twice.

```dmk
<div data-controller="modal" data={ "index": self.i }>
```

## `DataSet`

The type the lowered code builds, public so a helper can build one too.

| Item | Purpose |
|---|---|
| `DataSet::new()` | an empty set |
| `.insert(key, value)` | set `key`, as `data-key="value"` |
| `.insert_bare(key)` | set `key` with no value, as `data-key` |
| `.remove(key)` | drop `key`, whatever set it |
| `.is_empty()` | inspect |
| `.write_attrs(r)` | write every entry |
| `DataItem` | what a whole source implements |
| `DataValue` | what one value implements |

A `DataSet` also implements `AttrSpread`, so one built in Rust can be spliced
with `{...expr}` instead of passed as a prop.

## On a component

`data` is an ordinary prop name on a component tag, so `data={expr}` there
passes the value straight through to the field — which is exactly how a
component takes a data map from its caller and splats it onto its own root:

```rust
#[derive(Component)]
pub struct Modal {
    pub data: Option<Vec<(String, String)>>,
}
```

```dmk
<div data-controller="modal" data={self.data.as_deref()}><slot/></div>
```

The list and map forms assemble markup and have nowhere to go on a component, so
`data=[…]` and `data={ "k": v }` on a component tag are errors — the same rule
`class` follows.
