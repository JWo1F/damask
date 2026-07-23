+++
title = "The deploy feed"
summary = "Loops, conditionals, an empty state, and the Rust inside a brace tag."
+++

helm's second panel is a feed of recent deploys. It has everything the badge did
not: a list to walk, a flag to check per entry, a state for when the list is
empty, and a "and 4 more" line that only appears when there are more.

Start with the data. Add to `src/model.rs`:

```rust
/// A release of one service.
pub struct Deploy {
    pub service: String,
    pub version: String,
    pub author: String,
    pub minutes_ago: u32,
    pub rolled_back: bool,
}

impl Deploy {
    /// Coarse relative time — enough for a feed, no date library needed.
    pub fn when(&self) -> String {
        match self.minutes_ago {
            0 => "just now".to_string(),
            m if m < 60 => format!("{m}m ago"),
            m if m < 60 * 24 => format!("{}h ago", m / 60),
            m => format!("{}d ago", m / (60 * 24)),
        }
    }
}
```

## Borrowed props

The feed does not own the deploys — the page does. A component may borrow, and
the lifetime goes on the struct as it would anywhere else:

```rust
// src/deploy_feed.rs
use damask::Component;

use crate::model::Deploy;

#[derive(Component)]
pub struct DeployFeed<'a> {
    pub deploys: &'a [Deploy],
    /// How many entries to show; the rest are summarised as a remainder.
    pub limit: usize,
}

impl DeployFeed<'_> {
    pub fn visible(&self) -> &[Deploy] {
        let n = self.limit.min(self.deploys.len());
        &self.deploys[..n]
    }

    pub fn hidden(&self) -> usize {
        self.deploys.len().saturating_sub(self.limit)
    }
}
```

Those two methods are the seam. Slicing to a limit and clamping a subtraction are
things Rust does well and markup does badly, so they happen here and the template
calls them.

## The template

```dmk
<!-- src/deploy_feed.dmk -->
<section>
  <h1>Recent deploys</h1>
  {#if self.deploys.is_empty()}
    <p class="empty">Nothing has shipped in the last 24 hours.</p>
  {:else}
    <ul class="feed">
      {#for d in self.visible()}
        <li>
          <span class="svc">{d.service}</span>
          <span class="ver">{d.version}</span>
          <span class="owner">by {d.author}</span>
          {#if d.rolled_back}<span class="rb">rolled back</span>{/if}
          <span class="ago">{d.when()}</span>
        </li>
      {/for}
    </ul>
    {#if self.hidden() > 0}
      <p class="sub">and {self.hidden()} older deploy(s) not shown.</p>
    {/if}
  {/if}
</section>
```

Everything the language adds lives inside braces, and what is inside is **a Rust
block**, evaluated in the scope of the generated render method. It sees `self`,
the methods on the `impl` next door, and anything imported with `{use}` — which
chapter five needs and introduces. That is why `self.visible()`, `self.hidden() > 0` and `d.when()` need no special
support: they are not template expressions, they are Rust.

Because it is a block, the last expression is what prints — `{2 + 3; 10}` prints
`10` — and a tag that is a statement or a binding runs and prints nothing:

```dmk
{let total = self.deploys.len()}
<p>{total} deploys</p>
```

A literal brace is written as an expression: `{"{"}`.

## Conditionals

`{#if}` splices its condition into a Rust `if`, so `{:else if}`, `{:else}` and
`if let` all work:

```dmk
{#if let Some(error) = &self.error}
  <p class="error">{error}</p>
{/if}
```

## Loops

`{#for pat in E}` is a Rust `for` loop, spelled the way you already know. `E` is
anything iterable: `self.visible()` returns a slice, so `d` is a `&Deploy`; over
a field you own you would write `{#for d in &self.deploys}` for the same reason.
An index is `.enumerate()`, no different from a hand-written loop:

```dmk
{#for (i, d) in self.visible().iter().enumerate()}
  <li value={i + 1}>{d.service}</li>
{/for}
```

The binding is a real Rust pattern, so destructuring works:

```dmk
{#for (key, value) in &self.pairs}
  <dt>{key}</dt><dd>{value}</dd>
{/for}
```

## Comments

`<!-- … -->` passes through to the output. `{# … #}` does not, and it takes the
blank line it would otherwise leave behind with it — use it for a note to the
next person to open the file rather than to the browser.

## What does not work

**Control flow cannot appear in attribute position.** This is a parse error:

```dmk
<input {#if self.locked}disabled{/if}>
```

Attribute *names* are static; there is no `data-{key}=`. A conditional attribute
is expressed through its value instead, which is the next chapter.

## Running it

```rust
// src/main.rs
mod deploy_feed;
mod model;
mod status_badge;

use damask::Component;

use crate::deploy_feed::DeployFeed;
use crate::model::Deploy;

fn main() {
    let deploys = vec![
        Deploy {
            service: "checkout-api".into(),
            version: "v5.1.2".into(),
            author: "ada".into(),
            minutes_ago: 12,
            rolled_back: false,
        },
        Deploy {
            service: "image-resizer".into(),
            version: "v1.0.0-rc<1>".into(),
            author: "grace".into(),
            minutes_ago: 95,
            rolled_back: true,
        },
        Deploy {
            service: "edge-router".into(),
            version: "v2.14.0".into(),
            author: "linus".into(),
            minutes_ago: 1500,
            rolled_back: false,
        },
    ];

    let feed = DeployFeed {
        deploys: &deploys,
        limit: 2,
    };
    println!("{}", feed.render());
}
```

```sh
$ cargo run
<section>
  <h1>Recent deploys</h1>
  <ul class="feed">
    <li>
      <span class="svc">checkout-api</span>
      <span class="ver">v5.1.2</span>
      <span class="owner">by ada</span>

      <span class="ago">12m ago</span>
    </li>
    <li>
      <span class="svc">image-resizer</span>
      <span class="ver">v1.0.0-rc&lt;1&gt;</span>
      <span class="owner">by grace</span>
      <span class="rb">rolled back</span>
      <span class="ago">1h ago</span>
    </li>
  </ul>
  <p class="sub">and 1 older deploy(s) not shown.</p>
</section>
```

The second version reads `v1.0.0-rc&lt;1&gt;`. That release name contains a `<`,
and `{ … }` escaped it on the way out — the default, applied to every value a
template prints, whether or not you were thinking about where it came from. There
is a way to opt out, and chapter five needs it exactly once.

Set `limit: 5` and the remainder line disappears. Pass an empty slice and the
whole list is replaced by the empty state. That is the panel finished; next comes
the table beside it, which needs attributes that do more than hold a string.
