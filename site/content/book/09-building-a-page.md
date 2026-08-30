+++
title = "Beyond helm"
summary = "What changes when seven components become forty: project shape, a chrome value, tones, previews."
+++

helm is seven components in one directory, and at that size nothing about the
arrangement matters. This chapter is the second application: the conventions that
keep a view layer honest once there are forty, drawn from the two real ones the
project maintains — `examples/dashboard` in the repository, and the site you are
reading.

## A shape that works

```
src/view/
  layouts/     the document and the chrome around it
  ui/          the kit: badges, buttons, cards, tables
  pages/       one component per route
```

helm's `Page` is a layout, its `StatusBadge` is kit, and its `Dashboard` is a
page — the split was there from chapter five, it just had nowhere to live.

**Pages are the contract.** A page struct's fields are exactly what that route
must produce; there is no intermediate map of "template-visible keys". A handler
that stops supplying a field breaks the build.

**The kit takes a `class` prop.** A component owns its appearance and the call
site owns its placement:

```dmk
<Card tone={Tone::Danger} class="mt-6">…</Card>
```

which lands in the component's own class list, where an unset `Option` costs
nothing:

```dmk
<div class=["card", self.tone.skin(), self.class]>
```

## Derived values belong on the model

You have been doing this since chapter two. `Status::label`, `Fleet::worst`,
`Deploy::when` — every one of them keeps a `match` or a fold out of the markup
and puts it somewhere a unit test can reach. The rule is worth stating outright:
a template reads fields and calls methods; anything more than a field access or a
comparison goes on the type it describes.

The same applies to formatting. A `Display` impl on `Status` is what lets
`{svc.status}` print a label without the template knowing there is a match
behind it.

## One value for the chrome

helm's `Page` took five props, and only one of them — the content — was about the
page. The other four are what every page needs around it, and passing them
individually on every page struct is how they stop matching.

```rust
/// What every page needs around its own content.
#[derive(Debug, Clone)]
pub struct Chrome {
    pub title: String,
    pub current: &'static str,
    pub nav: Vec<NavEntry>,
    pub user: User,
}

#[derive(Component)]
pub struct Dashboard<'a> {
    pub chrome: Chrome,
    pub fleet: &'a Fleet,
}
```

It is assembled once per request and passed through untouched, so a page
rendering a table has no reason to name the fields of the header.

That shape is not a style preference — it is forced. Damask has **no ambient
context**: no thread-local, no implicit request object. Anything a deep component
needs travels down as a prop. The cost is real, and the compensation is that what
a page depends on is visible in its type.

## Tone, not color

An enum for semantic state, mapped to a skin by each component that renders it:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone { Ok, Warn, Danger, Muted }
```

```rust
impl Badge {
    fn skin(&self) -> &'static str {
        match self.tone {
            Tone::Ok => "bg-ok-bg text-ok",
            Tone::Warn => "bg-warn-bg text-warn",
            Tone::Danger => "bg-danger-bg text-danger",
            Tone::Muted => "bg-surface text-muted",
        }
    }
}
```

A badge needs a background and a text color where a dot needs only a fill, so
each maps the same word to its own thing. Passing class strings around instead is
how two badges end up a shade apart.

## Utility CSS

If you use Tailwind, point its scanner at the whole view tree:

```css
@import "tailwindcss" source(none);
@source "../view";
```

`source(none)` turns off automatic detection so that `@source` is the whole list
rather than an addition to it — otherwise every class-shaped string in the repo,
including prose in a README, becomes CSS in your bundle.

Both halves of a component are equally visible to the scanner: a skin written in
the `.dmk` and a skin written in the `impl` beside it are scanned alike, which is
what lets a component hold its class strings in whichever place reads better.

Two rules earn their keep. **No `@apply`** — a component's full appearance should
be readable at its definition, and `@apply` scatters it back into CSS and gives
up dead-class elimination. **Colors come from tokens** — `bg-surface`,
`text-muted`, not `bg-[#2563eb]` — so a palette change is one edit.

And remember the scanner caveat from
[The service table](/book/attributes/): `class:name={cond}` hides the name
in an attribute *name*, where Tailwind will not find it. Use the map form for
anything that must survive the build.

## Static previews

helm wrote its page to a file with one fixture in `main`. Scale that up and it is
the most useful tool in the project: a binary that builds a fixture for each page
and writes the HTML to disk gives you every page of the application in a browser
without running the application, without a database, and without a login.

Because a page struct is the whole contract, that harness stops compiling when a
page's props change — which is the harness working. A preview that still built
while the page it previews had moved on would be showing you software that no
longer exists.

This site is that idea taken one step further: the fixtures come from markdown on
disk instead of from Rust, and the output *is* the deliverable.

## What the book did not cover

helm never needed generic components, `#[component(default)]`, attribute
spreading, or a renderer of its own beyond the built-in one. Those exist, they
are small, and they are documented where they belong: [Props](/docs/props/),
[Attributes](/docs/attributes/), [Slots](/docs/slots/) and
[Renderers](/docs/renderers/). The [reference](/docs/) has every rule in lookup
order, and `examples/` in the repository has all of it as code that compiles —
including `examples/dashboard`, which is helm with more services in it.

The next page you build is your own. Start where this book started: a value worth
rendering, a struct beside a template, and one `cargo run` to prove it.
