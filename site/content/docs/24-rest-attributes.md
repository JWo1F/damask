+++
title = "Attributes a component does not name"
summary = "`#[prop(rest)]`: the bag every attribute that is not a prop lands in, and the rules that decide which is which."
section = "Components"
+++

A component tag may carry attributes the component has never heard of. They land
in one field, and reach the page wherever the component's template spreads it.

```rust
#[derive(Component, Default)]
#[component(default)]
pub struct Hidden {
    pub value: Option<String>,
    #[prop(rest)]
    pub attrs: Attrs,
}
```

```dmk
<input type="hidden" value={self.value.clone()} {...self.attrs}>
```

```dmk
<Hidden data-cover-target="input" aria-hidden="true" autofocus/>
```

All three attributes reach the `<input>`. Without the bag each would have to be
a declared prop, or the call site would have to hand the component a string of
markup to splice.

## The bag is opt-in

Only a component with a `#[prop(rest)]` field takes attributes it does not
declare. Everything else refuses them:

```
error[E0277]: this component takes only the props it declares
  |
6 |   <Card titel="Deploy finished"/>
  |         ^^^^^ there is no `#[prop(rest)]` field to carry an attribute the
  |               component does not name
```

That is the point of the opt-in. A component that accepts everything cannot tell
a passed-through attribute from a misspelled prop, so the typo above would be
rendered into the page instead of reported.

A struct has at most one such field, and its type is
[`Attrs`](#what-is-in-the-bag). It is never required, whatever
`#[component(default)]` says: a bag nothing was put into is an empty bag.

## Which attributes are props

A **declared prop wins.** `value` above is a prop, so `value="…"` is read by the
component and never reaches `attrs`. Everything else goes to the bag.

Nothing in the markup says which is which, and that is deliberate — a call site
writes HTML attributes and the component decides what it knows about. Two rules
follow from how the choice is actually made.

**It is made when the template compiles, not while it runs.** A template is
lowered in one crate and the component is compiled in another, so this pass
cannot see `Hidden`'s fields. It therefore does not decide: it emits the setter
call *and* a fallback of the same name that routes to the bag, and lets Rust's
method resolution choose between them. An inherent setter — a declared prop —
wins; a name with no setter falls through. The cost is a trait definition per
distinct attribute name, scoped to the one render function.

**A name that could not be a method is a bag attribute on sight.** That is every
hyphenated name (`data-cover-target`, `aria-label`, `x-on:click`) and every Rust
keyword (`type`, `for`, `async`), since no struct field could be called either.
So `<TextInput type="email"/>` works even though `type` can never be a prop.

## Which attributes reach the page

**An element's own attributes win over a spread.** A tag that writes `type`
itself and also spreads a bag holding `type` writes it once, from the tag. The
alternative is a duplicate attribute, which is not valid HTML and which browsers
resolve by a rule nobody writing the template was thinking about.

```dmk
<input type={self.kind()} {...self.attrs}>
```

Here a `type` at the call site is dropped. A component that wants a call site to
override something declares it as a prop — which is exactly what a prop is for.

The set of names skipped is the literal attribute names in that tag, decided when
the template compiles, so what it costs at run time is a scan of a list that is
usually empty. This applies to every `{...}` spread, not only to a bag: see
[Spreading](/docs/attributes/#spreading).

## What is in the bag

`Attrs` holds name/value pairs and nothing else. It is keyed the way
[`DataSet`](/docs/data-attributes/) is — a name given twice keeps its **first
position** and takes its **last value**, so a default a component filled in can
be overridden without the output reshuffling — and a name that could not be
written safely is dropped; see
[Names that cannot be written](/docs/attributes/#names-that-cannot-be-written).

A value is whatever `IntoAttrValue` accepts, which is the set
[`Attr`](/docs/attributes/#attr) covers and for the same reasons: a `bool`
renders a bare attribute or none at all, an `Option` renders nothing when it is
`None`, and there is no blanket impl over `Display` that would collide with
either.

```rust
let mut attrs = Attrs::new();
attrs.insert("aria-label", "Email address");
attrs.insert("autofocus", true);          // a bare attribute
attrs.insert("disabled", false);          // nothing at all
attrs.insert("readonly", None::<String>); // nothing at all

assert_eq!(attrs.get("aria-label"), Some("Email address"));
assert!(attrs.contains("autofocus"));     // …though `get` gives it no value
```

`get`, `contains` and `iter` are there so a component can read what it was
given — merge a class, refuse an attribute, count them — rather than only pass
it on.

## There is no markup in it

`Attrs` cannot hold a string. Attributes assembled into markup by hand were the
older spelling of this whole idea:

```dmk
<Hidden attrs={r#"data-cover-target="input""#}/>
```

and that is now a build failure, not a deprecation:

```
error[E0277]: `str` is not a set of attributes
  = note: attributes written as raw markup are no longer accepted — write them
          at the call site as attributes: `<Hidden data-controller="signup"/>`
```

The reason is escaping. A bag escapes each name and each value on its own, so a
quote inside a value cannot end the attribute and begin another; a string
splices whole and can promise none of that. `AttrSpread for &'static str` still
exists for [spreading onto an element](/docs/attributes/#spreading), where the
`'static` bound is the guarantee — but a prop is not a place markup belongs.

## Passing a whole set

`attrs={…}` takes a set rather than one attribute, through `AttrSet`: an `Attrs`,
a list of pairs, or an `Option` of either. So does `{...expr}` on a component
tag, which folds a set in where it was written:

```dmk
<Passthrough {...&self.tracking}/>
```

A wrapper forwards its own bag the same way, which is how a component that is
really a thin layer over another one stays transparent:

```dmk
<div class="field"><TextInput {...self.attrs}/></div>
```

## See also

- [Props](/docs/props/) — the typed half, and why a prop is the way to override
  what a component writes itself.
- [Attributes](/docs/attributes/) — `Attr`, spreading, and the name rules the bag
  inherits.
- `examples/attributes` in the repository, which is all of the above running.
