+++
title = "Token lists"
summary = "`{@tokens(…)}` — one space-separated value, assembled from parts."
section = "Templates"
+++

`{@tokens(…)}` builds a space-separated value out of pieces that each decide
whether they appear. `class` is what it is for most of the time, but the helper
is what asks for the list, not the attribute's name — so `rel`, `sandbox` and
`headers` take it on the same terms. ([`{@attrs(…)}`](/docs/attribute-groups/)
is the other helper, for the other job: one set expanded into many attributes.)

All of it combines on one element — a quoted value that interpolates, then a
helper, then a directive:

```dmk
<div class="card shadow"
     class={@tokens(self.extra, "base", "is-open": self.open)}
     class:is-loading={self.busy}>
```

## Entries

A positional entry is a value; a `name: cond` entry is a name and the condition
that keeps it.

| Entry | Contributes |
|---|---|
| a string (`&str`, `String`, `Cow<'_, str>`) | its whitespace-separated names |
| a slice, array or `Vec` of them | each one's names |
| an `Option` of any of those | those names when `Some` |
| `name: cond` | `name`, while `cond` holds |

```dmk
<div class={@tokens("card", self.tone.skin(), self.class)}>
```

Each positional entry is lowered to its own `TokenItem::add_to` call, so entries
need no common type. One entry may contribute several names — `"text-white
bg-brand"` splits on whitespace — which is what lets a variant resolve to a run
of classes at once.

A **literal** `None` is dropped at compile time: a bare `None` has no type to
infer from, so it cannot be left for runtime.

Names are **deduplicated**, keep their **first-mention order**, and an empty
result **omits** the attribute entirely rather than emitting `class=""`.

## Names

The name in a `name: cond` entry is text, not Rust. Write it as a bare
identifier where one will do, and quote it otherwise:

```dmk
<span class={@tokens(animate-pulse: self.live, "md:px-3": self.wide, "w-1/2": self.half)}>
```

A bare name may hold letters, digits, `_` and `-`. Anything else — the `:` of a
Tailwind variant, the `/` of a fraction, a `[` of an arbitrary value — has to be
quoted, because the first bare `:` in an entry is where its condition begins.

## Directive

`class:name={cond}` adds or removes exactly one name, and **takes precedence**
over whatever the helper produced. A bare `class:name` is always on. It is the
last attribute in Damask whose *name* means something.

```dmk
<div class={@tokens(self.base())} class:is-open={self.open} class:has-border>
```

The value must be bare or `{ … }`; `class:name="true"` is an error, as is a
`class:` with nothing after the colon. Directives work with or without a `class`
of their own — with one, the attribute is written where `class` stood; without,
where the first directive stood.

> [!WARNING]
> **CSS scanners cannot see a directive's class name.** It lives in the attribute
> *name* (`class:animate-pulse`), and Tailwind and friends scan for strings — so
> the rule gets compiled out of your stylesheet. When a class has to be
> discoverable, write it in the helper, where names are ordinary strings.

## Precedence

Everything lands in one `TokenList`, in this order, with later sources winning:

1. the quoted `class="…"`, or the `class={expr}`
2. the `{@tokens(…)}` entries, in order
3. `class:name` directives, in the order written

Because the list dedupes and keeps first-mention order, a directive that adds a
name already present is a no-op, and one that removes a name removes it whatever
contributed it.

## `TokenList`

The type the lowered code builds, public so a helper can build one too.

| Item | Purpose |
|---|---|
| `TokenList::new()` | an empty list |
| `.add(text)` | add every whitespace-separated name in `text` |
| `.set(text, on)` | add or remove them, per a directive |
| `.is_empty()`, `.to_value()` | inspect, or join with spaces |
| `.into_value()` | the joined value, or `None` when empty |
| `.write_attr(name, r)` | write ` name="…"`, or nothing when empty |
| `TokenItem` | what an entry implements: `str`, `String`, `Cow`, slices, `Vec`, `Option<T>`, `&T` |

## On a component

`{@tokens(…)}` builds a *value*, which is what a prop takes — so a call site can
hand a component the same conditional list an element gets, and the component
holds a `String`:

```dmk
<Button class={@tokens("w-full", "is-busy": self.saving)}/>
```

For an attribute the component does not name, the value reaches its
[bag](/docs/rest-attributes/) as an `Option<String>`, so an empty list writes no
attribute at all.

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
<button class={@tokens(self.variant.skin(), self.class)}><slot/></button>
```
