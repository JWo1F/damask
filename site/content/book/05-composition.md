+++
title = "Assembling the page"
summary = "Nesting components, the chrome around them, and a document shell with a hole in it."
+++

Three panels exist and nothing joins them. This chapter turns them into a page:
the badge goes into the table, the table and the feed go into a dashboard, and
the dashboard goes inside a document shell that knows nothing about it.

## A capitalized tag is a component

Lowercase tags are HTML. Anything starting with an uppercase letter is built from
its attributes and rendered:

```dmk
<!-- src/service_table.dmk -->
<table>
  {use crate::status_badge::StatusBadge}
  …
        <td><StatusBadge status={svc.status}/></td>
```

`{use …}` is an ordinary Rust `use`, and it is **scoped to the element that
encloses it** — inside `<table>` here, so `StatusBadge` is in scope for the whole
table and nowhere else. At the top of a file, before any element, an import
covers everything, which is where most of them go.

The tag is checked like the struct literal it becomes: a missing required prop is
a compile error naming it, and a prop of the wrong type is a type error. Values
**move** into the component's fields, so a `String` you still need is passed with
`.clone()` and a `Copy` type like `Status` needs nothing. That is ordinary Rust
ownership, not a template rule.

A component you already hold as a value needs no tag at all — `{@render}` writes
it straight into the current renderer:

```dmk
<section>{@render self.badge}</section>
```

Rerun `cargo run` and the status cells are pills. Two components, one output
buffer: the child writes into the same renderer as its parent, so nothing is
rendered to an intermediate `String` and escaping cannot differ between them.

## Props a caller may skip

A prop must be passed unless its **type** says what leaving it out means, and
only `Option<_>` does. A skipped one arrives as `None`:

```rust
#[derive(Component)]
pub struct Notice {
    pub title: String,             // required
    pub detail: Option<String>,    // skippable
    pub dismissible: Option<bool>, // a flag a caller may omit
}
```

```dmk
<Notice title="Deploy finished"/>
<Notice title="Rollback" detail="check the log" dismissible/>
```

A **quoted** value converts into the `Option` for you — `detail="check the log"`
arrives as `Some`. A `{ … }` value stays exact, so forwarding one is
`detail={self.detail.clone()}`. That exactness is what lets `count={2 + 8}` infer
to the prop's integer type.

> [!NOTE]
> `bool` is required like any other type. A flag a caller may leave out is
> `Option<bool>`, not `bool`. If a struct's defaults are meaningful rather than
> zero values, `#[component(default)]` makes every prop skippable and fills the
> gaps from `Default` — see [Props](/docs/props/).

## The chrome

helm's masthead and footer both summarise the whole fleet, so the fleet becomes a
type with the rollups on it. In `src/model.rs`:

```rust
/// The whole fleet, plus the rollups the page headlines.
pub struct Fleet {
    pub services: Vec<Service>,
    pub deploys: Vec<Deploy>,
    /// The availability target every service is measured against.
    pub slo_target: f64,
}

impl Fleet {
    pub fn count(&self, status: Status) -> usize {
        self.services.iter().filter(|s| s.status == status).count()
    }

    /// The fleet headline: the most severe status any service is in. An empty
    /// fleet is reported healthy — there is nothing broken in it.
    pub fn worst(&self) -> Status {
        self.services.iter().map(|s| s.status).max().unwrap_or(Status::Healthy)
    }

    pub fn avg_uptime_label(&self) -> String {
        let n = self.services.len();
        if n == 0 {
            return "100.000%".to_string();
        }
        format!("{:.3}%", self.services.iter().map(|s| s.uptime_pct).sum::<f64>() / n as f64)
    }

    pub fn slo_label(&self) -> String {
        format!("{:.2}%", self.slo_target)
    }

    pub fn breaching(&self) -> usize {
        self.services.iter().filter(|s| s.breaches_slo(self.slo_target)).count()
    }

    /// Whether anything needs attention — drives the banner in the header.
    pub fn all_clear(&self) -> bool {
        self.worst() == Status::Healthy && self.breaching() == 0
    }
}
```

`worst()` is why `Status` was declared healthy-first back in chapter two: the
derived `Ord` makes `max()` mean "most severe".

```rust
// src/site_header.rs
use damask::Component;

use crate::model::Fleet;

#[derive(Component)]
pub struct SiteHeader<'a> {
    pub fleet: &'a Fleet,
    pub nav: Vec<&'static str>,
    /// Which entry in `nav` is the current page.
    pub current: &'static str,
}

impl SiteHeader<'_> {
    /// `href` for a nav entry — the landing page lives at `/`, the rest at
    /// their lowercased name.
    pub fn href(&self, entry: &str) -> String {
        if entry == "Overview" {
            "/".to_string()
        } else {
            format!("/{}", entry.to_lowercase())
        }
    }
}
```

```dmk
<!-- src/site_header.dmk -->
<header class="masthead">
  <div class="wrap">
    {use crate::model::Status}
    <div class="masthead-row">
      <div class="brand">helm <span>/ fleet status</span></div>
      <nav>
        {#each &self.nav as entry}
          <a href={self.href(entry)} class={ "active": *entry == self.current }>{entry}</a>
        {/each}
      </nav>
    </div>
    {#if self.fleet.all_clear()}
      <p class="banner ok">All {self.fleet.services.len()} services are healthy and meeting the {self.fleet.slo_label()} availability target.</p>
    {:else if self.fleet.worst() == Status::Down}
      <p class="banner alert">{self.fleet.count(Status::Down)} service(s) down — {self.fleet.breaching()} of {self.fleet.services.len()} are below the {self.fleet.slo_label()} target.</p>
    {:else}
      <p class="banner alert">Degraded: {self.fleet.breaching()} of {self.fleet.services.len()} services are below the {self.fleet.slo_label()} target.</p>
    {/if}
  </div>
</header>
```

The footer is the same shape and shorter:

```rust
// src/site_footer.rs
#[derive(Component)]
pub struct SiteFooter<'a> {
    pub fleet: &'a Fleet,
    pub commit: String,
    pub year: u32,
}

impl SiteFooter<'_> {
    /// Short commit hash, as a footer would show it.
    pub fn short_commit(&self) -> &str {
        let n = self.commit.len().min(7);
        &self.commit[..n]
    }
}
```

```dmk
<!-- src/site_footer.dmk -->
<footer>
  <div class="wrap row">
    <span>{self.fleet.services.len()} services · mean uptime {self.fleet.avg_uptime_label()} · target {self.fleet.slo_label()}</span>
    <span>© {self.year} helm · build {self.short_commit()}</span>
  </div>
</footer>
```

## The shell, and the hole in it

The document is a component like any other. What makes it a *shell* is that its
middle is not its own:

```dmk
<!-- src/page.dmk -->
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
    <title>{self.title}</title>
    <style>{@html crate::theme::CSS}</style>
  </head>
  <body>
    {use crate::site_footer::SiteFooter}
    {use crate::site_header::SiteHeader}
    <SiteHeader fleet={self.fleet} nav={self.nav.clone()} current={self.current}/>
    <main class="wrap"><slot/></main>
    <SiteFooter fleet={self.fleet} commit={self.commit.clone()} year={self.year}/>
  </body>
</html>
```

```rust
// src/page.rs
#[derive(Component)]
pub struct Page<'a> {
    pub title: String,
    pub fleet: &'a Fleet,
    pub nav: Vec<&'static str>,
    pub current: &'static str,
    pub commit: String,
    pub year: u32,
}
```

There is no layout mechanism here to learn, because there is no layout mechanism.
A document is a component; a page nests inside it the same way a badge nests
inside a row.

Two details in that template. `{@html …}` prints without escaping, which is what
a stylesheet needs — and it is the opt-out chapter three mentioned. Use it for
content you produced yourself: a `include_str!`-ed asset, a child's `.render()`,
markdown compiled at build time. Anything derived from a request goes through
`{ … }`. The asymmetry is the point: the dangerous form is longer, named, and
obvious in a diff.

```rust
// src/theme.rs — with your CSS in src/theme.css beside it
pub const CSS: &str = include_str!("theme.css");
```

> [!NOTE]
> The stylesheet cannot live inside the `<style>` block as literal text: `.dmk`
> has no raw-text elements, so a `{` in a CSS rule would open a tag. Keeping it
> in a `.css` file means your editor treats it as CSS, which you wanted anyway.

And `<slot/>` is the hole. It marks where a caller's content lands, and it is not
a field: the struct above has no `Render` parameter and no `children` prop, and
the template could add a second slot without touching it.

## Filling a slot

A `<slot>` may be named, and its body is the **fallback** rendered when nothing
fills it:

```dmk
<!-- frame.dmk -->
<section class="frame">
  <h2>{self.title}</h2>
  <slot/>
  <footer><slot name="footer">© anon</slot></footer>
</section>
```

A caller routes content with `slot="…"` on a **direct child** — the same pairing
a browser custom element uses. The whole element goes in, several children may
name the same slot and land in written order, and the `slot` attribute itself is
consumed rather than rendered:

```dmk
{use crate::frame::Frame}
<Frame title={self.heading.clone()}>
  <p>{self.body}</p>                          <!-- fills the default slot -->
  <span slot="footer">© {self.year}</span>
  <a slot="footer" href="/about">About</a>
</Frame>
```

Outside a component element, `slot` is an ordinary attribute and passes through,
so a template can still address a browser-side custom element's shadow slots.

> [!IMPORTANT]
> Slots are matched **by name at render time**. A misspelled `name` renders the
> fallback instead of failing the build, and filling a slot the template does not
> declare is silently ignored. This is the one compile-time check Damask trades
> away, and it is what buys slots that never appear on the struct.

## Filling it from Rust

helm does not wrap its dashboard in a template, because the shell's whole point
is that it does not know what it wraps — a second page would fill the same slot
with something else. The choice belongs where both values exist, which is `main`.

First the content, which is now just a composition of the last two chapters:

```rust
// src/dashboard.rs
#[derive(Component)]
pub struct Dashboard<'a> {
    pub fleet: &'a Fleet,
    /// How many deploys the feed shows before summarising the remainder.
    pub feed_limit: usize,
}
```

```dmk
<!-- src/dashboard.dmk -->
<section>
  {use crate::deploy_feed::DeployFeed}
  {use crate::service_table::ServiceTable}
  <h1>Fleet overview</h1>
  <p class="sub">{self.fleet.services.len()} services · mean uptime {self.fleet.avg_uptime_label()} · worst {self.fleet.worst()}</p>
  <ServiceTable services={&self.fleet.services} slo_target={self.fleet.slo_target}/>
  <DeployFeed deploys={&self.fleet.deploys} limit={self.feed_limit}/>
</section>
```

Then `main` builds both and hands one to the other — `demo_fleet()` being a
function that returns a `Fleet` holding the services and deploys the last two
chapters used:

```rust
use damask::{Component, DEFAULT_SLOT, Slot, Slots};

fn main() {
    let fleet = demo_fleet();
    let page = Page {
        title: "Fleet status · helm".into(),
        fleet: &fleet,
        nav: vec!["Overview", "Services", "Incidents"],
        current: "Overview",
        commit: "9f3c1ab7d20e".into(),
        year: 2026,
    };
    let dashboard = Dashboard {
        fleet: &fleet,
        feed_limit: 2,
    };
    println!(
        "{}",
        page.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &dashboard)]))
    );
}
```

`render_with` is `render` plus a set of fills. `DEFAULT_SLOT` is the empty
string — the name of the slot that a bare `<slot/>` places — so a named slot can
never collide with it.

The fills are **borrowed**, not owned: a `Slot` holds a `&dyn Render` and `Slots`
borrows the slice. `dashboard` stays on `main`'s stack, borrows the same `fleet`
the page borrows, and is never rendered to a string on the way in.

## Running it

```sh
$ cargo run
<!DOCTYPE html>
<html lang="en">
  …
  <body>
    <header class="masthead">
      …
      <a href="/" class="active">Overview</a>
      <a href="/services">Services</a>
      …
      <p class="banner alert">Degraded: 1 of 2 services are below the 99.90% target.</p>
    </header>
    <main class="wrap"><section>
      <h1>Fleet overview</h1>
      …
      <td><span class="badge degraded" data-status="degraded">Degraded</span></td>
      …
    </section></main>
    <footer>…</footer>
  </body>
</html>
```

Redirect it to a file, open it in a browser, and helm is a page. Two nav links
have no `class` attribute at all, for the same reason the first table row did
not.

What is missing is the row of rollup tiles above the table — four boxes,
identical but for their numbers. Writing that as a fifth component is more
ceremony than it deserves, and the next chapter has something cheaper.
