+++
title = "Your first component"
summary = "Start the project, write a struct and a template beside it, and render a status badge."
+++

helm starts with the smallest thing on the page: the pill that says whether one
service is up.

## The project

```sh
cargo new helm
```

```toml
[dependencies]
damask = "0.2"
```

Damask needs Rust 1.88 or newer. That is the whole setup: nothing to register, no
template directory to declare, no build script.

## Something to render

A component renders a value, so the value comes first. `src/model.rs` holds the
domain — for now, one enum:

```rust
// src/model.rs
use std::fmt::{self, Display};

/// Operational state of a service, ordered healthy → worst.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Healthy,
    Degraded,
    Down,
}

impl Status {
    /// Human-readable form, for badges and summaries.
    pub fn label(self) -> &'static str {
        match self {
            Status::Healthy => "Healthy",
            Status::Degraded => "Degraded",
            Status::Down => "Down",
        }
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
```

The `Display` impl is doing more work than it looks like. It lets a template
print a status without knowing there is a `match` behind it, and it is the first
instance of a rule this book keeps returning to: **markup reads values, Rust
computes them.**

## The two-file rule

A component is **two files that share a basename, in the same directory**:

```
src/
  model.rs
  status_badge.rs     the struct
  status_badge.dmk    the template
```

```rust
// src/status_badge.rs
use damask::Component;

use crate::model::Status;

/// A status pill.
#[derive(Component)]
pub struct StatusBadge {
    pub status: Status,
}
```

```dmk
<!-- src/status_badge.dmk -->
<span class="badge">{self.status}</span>
```

The struct's **fields are its props**, and the template reads them off `self`. A
`{ … }` tag prints its value, HTML-escaped. The derive leaves the struct itself
alone — generics, other derives and doc comments all work as usual — and adds
`Render` and `Component` impls beside it.

The template is found by the *struct's* name, lowercased to snake_case, next to
the file the struct is declared in. `struct StatusBadge` looks for
`status_badge.dmk`. Editing that file triggers a rebuild on its own; there is no
`include_str!` to remember and no `build.rs` to configure.

> [!IMPORTANT]
> Create and edit the two as a pair. Renaming the struct means renaming the
> template, and a `#[derive(Component)]` with no `.dmk` beside it is a compile
> error rather than an empty render.

Two components may share a `.rs` file — resolution is by struct name, not by file
name — and if the pairing has to be broken entirely, `#[template(path = "…")]`
names a file instead. [The Component derive](/docs/derive/) has the resolution
rules in full.

## Running it

```rust
// src/main.rs
mod model;
mod status_badge;

use damask::Component;

use crate::model::Status;
use crate::status_badge::StatusBadge;

fn main() {
    let badge = StatusBadge {
        status: Status::Degraded,
    };
    println!("{}", badge.render());
}
```

```sh
$ cargo run
<span class="badge">Degraded</span>
```

`render()` comes from the `Component` trait, which is why `use damask::Component`
is at the top of `main.rs`. It returns a `String`.

Two things worth noticing about that build. A misspelled field — `statuss` — is a
compile error naming the field, because the generated code is a struct literal
and a field access like any other; nothing is looked up in a map at runtime. And
`{self.status}` compiled only because `Status` implements `Display`. A value that
cannot be printed is an error at the tag, not a blank space in the page.

One component, one field, one line of markup. The next chapter renders something
with a shape to it: a feed with a loop, a conditional, and an empty state.
