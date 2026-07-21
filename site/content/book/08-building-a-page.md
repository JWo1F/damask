+++
title = "Building a real page"
summary = "How the pieces settle into a project: a kit, layouts, page structs, and the conventions that keep them honest."
+++

Everything so far has been one component at a time. This chapter is about what a
codebase full of them looks like — drawn from the two real ones the project
maintains: `examples/dashboard` in the repository, and the site you are reading.

## A shape that works

```
src/view/
  layouts/     the document and the chrome around it
  ui/          the kit: badges, buttons, cards, tables
  pages/       one component per route
```

**Pages are the contract.** A page struct's fields are exactly what that route
must produce — no intermediate map of "template-visible keys". That means a
handler that stops supplying a field breaks the build, and a static render
harness built from those same structs cannot drift from the pages it previews.

**The kit takes a `class` prop.** A component owns its own appearance and the
call site owns its placement:

```dmk
<Card tone={Tone::Danger} class="mt-6">…</Card>
```

## One value for the chrome

Every page needs the same things around it — the title, the navigation, the
signed-in user. Passing those as a dozen props on every page struct is how they
stop matching.

```rust
/// What every page needs around its own content.
#[derive(Debug, Clone)]
pub struct Shell {
    pub page: &'static str,
    pub title: String,
    pub nav: Vec<NavEntry>,
    pub user: User,
}

#[derive(Component)]
pub struct Dashboard {
    pub shell: Shell,
    pub metrics: Vec<Metric>,
}
```

It is assembled once per request and passed through untouched, so a page
rendering a table has no reason to name the fields of the header.

Damask has **no ambient context** — no thread-local, no implicit request object.
Anything a deep component needs travels down as a prop. In practice that pushes
you towards a shell value like this one, which is a good outcome: what a page
depends on is visible in its type.

## Tone, not colour

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

A badge needs a background and a text colour where a dot needs only a fill, so
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

Two rules earn their keep:

- **No `@apply`.** A component's full appearance should be readable at its
  definition. `@apply` scatters it back into CSS and gives up dead-class
  elimination.
- **Colours come from tokens** — `bg-surface`, `text-muted` — not `bg-[#2563eb]`.
  A palette change should be one edit.

And remember the scanner caveat from the attributes chapter: `class:name={cond}`
hides the name in an attribute *name*, where Tailwind will not find it. Use the
map form for anything that must survive the build.

## Testing a component

Render it and assert on the markup. The interesting assertions are usually about
what is **absent**:

```rust
#[test]
fn a_plain_button_carries_no_action_attributes() {
    let out = Button { disabled: false, name: None }.render();
    // Inspect the attributes before `class`: the class list itself
    // legitimately contains `disabled:opacity-50`.
    let attrs = out.split_once(" class=").unwrap().0;
    assert!(!attrs.contains("disabled"), "{out}");
    assert!(!attrs.contains("name="), "{out}");
}
```

For a component whose content is a slot, build the children with `fragment` and
call `render_with` — see [Snippets and fragments](/book/snippets/).

## Static previews

Because a page struct is the whole contract, a binary that builds a fixture for
each page and writes the HTML to disk gives you every page of the application in
a browser without running the application. When a page's props change, that
harness stops compiling — which is the harness working. A preview that still
built while the page it previews had moved on would be showing you software that
no longer exists.

This site is that idea taken one step further: the fixtures come from markdown on
disk instead of from Rust, and the output *is* the deliverable.
