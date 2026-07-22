+++
title = "Your first component"
summary = "Adding the dependency, writing the pair, and the rule that finds them."
+++

Damask needs Rust 1.88 or newer and no build script.

```toml
[dependencies]
damask = "0.2"
```

## The two-file rule

A component is **two files that share a basename, in the same directory**:

```
src/ui/
  button.rs     the struct
  button.dmk    the template
```

The derive resolves the template by the *struct's* name, lowercased to
snake_case, next to the file the struct is declared in. `struct Button` looks for
`button.dmk`; `struct PageCard` looks for `page_card.dmk`.

> [!IMPORTANT]
> Always create and edit the two as a pair. Renaming the struct means renaming
> the template, and a `#[derive(Component)]` with no `.dmk` beside it is a
> compile error rather than an empty render.

Two components may share a `.rs` file — the resolution is by struct name, not by
file name — which is how a table and its cell can live together and still have
`table.dmk` and `cell.dmk`. What is *not* allowed is two templates with the same
basename in one directory.

## Writing it

```rust
// button.rs
use damask::Component;

#[derive(Component)]
pub struct Button {
    pub label: String,
    pub disabled: bool,
}
```

```dmk
<!-- button.dmk -->
<button type="button" disabled={self.disabled}>{self.label}</button>
```

The struct's **fields are its props**, and the template reads them off `self`.
The derive leaves the struct itself untouched — generics, other derives and doc
comments all work as usual — and adds `Render` and `Component` impls beside it.

## Rendering it

```rust
use damask::Component;

let markup = Button {
    label: "Save".into(),
    disabled: false,
}
.render();

assert_eq!(markup, r#"<button type="button">Save</button>"#);
```

Note what `disabled={self.disabled}` did with `false`: it emitted **nothing**. In
HTML an attribute's presence is what disables a control, so `disabled="false"`
would still be disabled. Damask asks the value's type how to appear, and a `bool`
renders a bare attribute or none at all. The next chapters cover that in full.

## Methods

Anything you would rather not write in markup goes in an ordinary `impl`:

```rust
impl Button {
    fn skin(&self) -> &'static str {
        if self.disabled { "btn btn--off" } else { "btn" }
    }
}
```

```dmk
<button class={self.skin()} disabled={self.disabled}>{self.label}</button>
```

This is the seam that keeps templates readable. A condition with three arms is a
`match` in Rust and a mess in markup, and putting it in the `impl` costs nothing
— it is the same file's worth of component either way.

## Editing the template

Changing a `.dmk` triggers a rebuild on its own. There is no build script to
configure and no `include_str!` to remember; the derive registers the template as
a dependency of the crate.

## Choosing a different template

If the pairing has to be broken — two components of the same name in one
directory, a template generated elsewhere — name it explicitly:

```rust
#[derive(Component)]
#[template(path = "button_compact.dmk")]
pub struct CompactButton {
    pub label: String,
}
```

The path resolves next to the struct, like the default does.
