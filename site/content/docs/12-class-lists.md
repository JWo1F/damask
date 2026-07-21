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
| a string (`&str`, `String`) | the name |
| an `Option` of a string | the name when `Some` |
| a map `{ "name": cond, … }` | each name whose condition is true |

```dmk
<div class=["card", self.tone.skin(), self.class]>
```

A **literal** `None` is dropped at compile time — a bare `None` has no type to
infer from, so it cannot be left for runtime.

Names are **deduplicated**, keep their **first-mention order**, and an empty
result **omits** the attribute entirely rather than emitting `class=""`.

## Map alone

`class={ "name": cond }` is the map form on its own. It is told apart from an
ordinary `class={expr}` by a top-level `:` that is not part of a `::` path.

```dmk
<span class={ "animate-pulse": self.live, "opacity-60": self.stale }>
```

## Directive

`class:name={cond}` adds or removes exactly one name, and **takes precedence**
over whatever the list produced. A bare `class:name` is always on.

```dmk
<div class=[self.base()] class:is-open={self.open} class:has-border>
```

> [!CAUTION]
> **CSS scanners cannot see a directive's class name.** It lives in the attribute
> *name* (`class:animate-pulse`), and Tailwind and friends scan for strings — so
> the rule gets compiled out of your stylesheet. When a class has to be
> discoverable, use the map form, whose names are ordinary strings.

## Precedence

Sources are combined in this order, with later ones winning:

1. the quoted `class="…"`
2. the `class=[…]` list, in entry order
3. `class:name` directives

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
