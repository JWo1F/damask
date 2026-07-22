+++
title = "Class lists"
summary = "The three extra forms `class` accepts, and how they combine."
section = "Templates"
+++

`class` is the one attribute with forms of its own, because building a class
string is the thing templates do most.

```dmk
<div class="card shadow"                                   <!-- quoted, interpolating -->
     class=[self.extra, "base", { "is-open": self.open }]  <!-- list -->
     class:is-loading={self.busy}>                         <!-- directive -->
```

## List

`class=[…]` takes entries of three kinds:

| Entry | Contributes |
|---|---|
| a string (`&str`, `String`, `Cow<'_, str>`) | its whitespace-separated names |
| an `Option` of a string | those names when `Some` |
| a map `{ "name": cond, … }` | each name whose condition is true |

```dmk
<div class=["card", self.tone.skin(), self.class]>
```

Each entry is lowered to its own `ClassItem::add_to` call, so entries need no
common type. One entry may contribute several names — `"text-white bg-brand"`
splits on whitespace — which is what lets a variant resolve to a run of classes
at once.

A **literal** `None` is dropped at compile time: a bare `None` has no type to
infer from, so it cannot be left for runtime.

Names are **deduplicated**, keep their **first-mention order**, and an empty
result **omits** the attribute entirely rather than emitting `class=""`.

## Map alone

`class={ "name": cond }` is the map form on its own. It is told apart from an
ordinary `class={expr}` by a top-level `:` that is not part of a `::` path —
"top level" meaning outside every bracket, string and char literal, so
`{ "a": matches!(x, Foo::B) }` is a map and `{Foo::B.skin()}` is not.

```dmk
<span class={ "animate-pulse": self.live, "opacity-60": self.stale }>
```

A map may also sit inside a list, which is how the two compose:
`class=["base", { "is-open": self.open }]`.

## Directive

`class:name={cond}` adds or removes exactly one name, and **takes precedence**
over whatever the list produced. A bare `class:name` is always on.

```dmk
<div class=[self.base()] class:is-open={self.open} class:has-border>
```

The value must be bare or `{ … }`; `class:name="true"` is an error, as is a
`class:` with nothing after the colon. Directives work with or without a `class`
of their own — with one, the attribute is written where `class` stood; without,
where the first directive stood.

> [!WARNING]
> **CSS scanners cannot see a directive's class name.** It lives in the attribute
> *name* (`class:animate-pulse`), and Tailwind and friends scan for strings — so
> the rule gets compiled out of your stylesheet. When a class has to be
> discoverable, use the map form, whose names are ordinary strings.

## Precedence

Everything lands in one `ClassList`, in this order, with later sources winning:

1. the quoted `class="…"`, or the `class={expr}`
2. the `class=[…]` list, in entry order
3. `class:name` directives, in the order written

Because the list dedupes and keeps first-mention order, a directive that adds a
name already present is a no-op, and one that removes a name removes it whatever
contributed it.

## `ClassList`

The type the lowered code builds, public so a helper can build one too.

| Item | Purpose |
|---|---|
| `ClassList::new()` | an empty list |
| `.add(text)` | add every whitespace-separated name in `text` |
| `.set(text, on)` | add or remove them, per a directive |
| `.is_empty()`, `.to_value()` | inspect, or join with spaces |
| `.write_attr(name, r)` | write ` class="…"`, or nothing when empty |
| `ClassItem` | what an entry implements: `str`, `String`, `Cow`, `Option<T>`, `&T` |

## On a component

A class list assembles markup, and a component prop is a value. `class=[…]` on a
component tag is an error; pass a string with `class={…}` or `class="…"` and let
the component's own template put it in a list.

## Where a skin lives

A class string may be written in the template or returned from a method on the
`impl` beside it. Both halves of a component are ordinary source, so a CSS
scanner pointed at the view tree sees them alike:

```rust
impl Variant {
    fn skin(self) -> &'static str {
        match self {
            Variant::Primary => "text-white bg-brand hover:bg-brand-strong",
            Variant::Secondary => "text-ink bg-surface border-line",
        }
    }
}
```

```dmk
<button class=[self.variant.skin(), self.class]><slot/></button>
```
