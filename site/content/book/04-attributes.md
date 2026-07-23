+++
title = "The service table"
summary = "Classes that assemble themselves from conditions, and attributes that know they are HTML."
+++

The centre of the page is a table of services — one row each, striped, with rows
that miss their availability target called out. Almost all of the work in it
happens in attributes.

The data first. `Status` gains a machine-readable form, and `Service` joins it in
`src/model.rs`:

```rust
impl Status {
    /// Machine-readable form, for CSS class suffixes and `data-` values.
    pub fn slug(self) -> &'static str {
        match self {
            Status::Healthy => "healthy",
            Status::Degraded => "degraded",
            Status::Down => "down",
        }
    }
}

/// Latency at or above this is called out in the table.
const SLOW_MS: u32 = 300;

/// One service in the fleet.
pub struct Service {
    pub name: String,
    pub owner: String,
    pub status: Status,
    /// Availability over the trailing window, as a percentage (`99.982`).
    pub uptime_pct: f64,
    /// p95 response time in milliseconds.
    pub latency_ms: u32,
    pub version: String,
}

impl Service {
    pub fn uptime(&self) -> String {
        format!("{:.3}%", self.uptime_pct)
    }

    pub fn latency(&self) -> String {
        format!("{} ms", self.latency_ms)
    }

    /// Whether p95 latency is bad enough to flag even when the service is up.
    pub fn is_slow(&self) -> bool {
        self.latency_ms >= SLOW_MS
    }

    /// Services that are down, or up but missing their availability target.
    pub fn breaches_slo(&self, target_pct: f64) -> bool {
        self.status == Status::Down || self.uptime_pct < target_pct
    }
}
```

## The problem: assembling a class

Rows want two conditional names: `alt` on every other row, `breach` on rows below
target. Written in Rust that is a small pile of string handling:

```rust
impl ServiceTable<'_> {
    fn row_class(&self, service: &Service, index: usize) -> String {
        let mut classes = String::new();
        if index % 2 == 1 {
            classes.push_str("alt");
        }
        if service.breaches_slo(self.slo_target) {
            if !classes.is_empty() {
                classes.push(' ');
            }
            classes.push_str("breach");
        }
        classes
    }
}
```

Half of that method is about the space between the names, and when both
conditions are false it returns `""` — which, dropped into `class={…}`, renders
`class=""`: an attribute that says nothing, on every second row.

## The fix: a class map

`class` — and only `class` — takes a **map of name to condition**, as many pairs
as the element needs:

```dmk
<tr class={ "alt": i % 2 == 1, "breach": svc.breaches_slo(self.slo_target) }>
```

Each name appears when its condition holds. They are joined with the space you
did not have to write, and when nothing holds the attribute is omitted
entirely — not emitted empty. Delete `row_class`; nothing else needs it.

That last property is why this beats a helper returning a `String`. The helper
cannot decline to be an attribute; the map can.

Where some names are unconditional, `class` takes a **list** instead, whose
entries may be strings, `Option`s of strings, or a map. The badge at the end of
this chapter is one:

```dmk
<span class=["badge", self.status.slug()]>
```

Names across a list are deduplicated and keep their first-mention order.

## The table

```rust
// src/service_table.rs
use damask::Component;

use crate::model::Service;

#[derive(Component)]
pub struct ServiceTable<'a> {
    pub services: &'a [Service],
    pub slo_target: f64,
}
```

```dmk
<!-- src/service_table.dmk -->
<table>
  <thead>
    <tr><th>Service</th><th>Status</th><th>Uptime</th><th>p95</th><th>Version</th></tr>
  </thead>
  <tbody>
    {#for (i, svc) in self.services.iter().enumerate()}
      <tr class={ "alt": i % 2 == 1, "breach": svc.breaches_slo(self.slo_target) }>
        <td><div class="svc">{svc.name}</div><div class="owner">{svc.owner}</div></td>
        <td>{svc.status}</td>
        <td>{svc.uptime()}</td>
        <td data-slow={svc.is_slow()}>{svc.latency()}</td>
        <td class="ver">{svc.version}</td>
      </tr>
    {/for}
  </tbody>
</table>
```

## An attribute asks its value

`data-slow={svc.is_slow()}` is a `bool`, and it renders a bare `data-slow` or
nothing at all. It never renders `data-slow="false"`, which would be a disaster:
a CSS rule matching `[data-slow]` matches the attribute's *presence*, so every
fast service would be flagged.

That is the general rule. On an HTML element, `attr={expr}` does not stringify
the value — it asks the value's *type* how an attribute should appear:

| Value type | Renders |
|---|---|
| `bool` | a bare ` attr` when true, **nothing** when false |
| `Option<T>` | nothing when `None`, otherwise as `T` |
| `&str`, `String`, `Cow<str>`, numbers, `char` | ` attr="escaped"` |

```dmk
<input name="email"
       required={self.required}
       placeholder={self.hint.clone()}
       maxlength={self.limit}>
```

With `required: false`, `hint: None` and `limit: Some(80)`, that renders
`<input name="email" maxlength="80">`.

> [!WARNING]
> Never write `disabled="{self.locked}"`. A **quoted** value always produces the
> attribute, and in HTML presence is what disables a control — so
> `disabled="false"` is a disabled control. Use a `bool` and no quotes.

There is no blanket `Display` impl behind this, deliberately: it would collide
with the `bool` and `Option` impls, which are the point. A type of your own
either implements `Attr` or reaches the template as a string.

Quoted values do interpolate, and the interpolated parts are escaped by the same
policy as `{ … }`:

```dmk
<tr title="row {i} of {self.services.len()}">
```

Reach for the quoted form when you are building a string, and for `{ … }` when
you are passing a value.

## Directives

A **directive**, `class:name={cond}`, adds or removes one name and takes
precedence over whatever the map or list produced. A bare `class:name` is always
on. `Control`, in the repository's `examples/showcase`, puts every form in one
tag:

```dmk
<input disabled={self.disabled}
       placeholder={self.placeholder}
       class=[self.extra, "base", { "invalid": self.invalid }]
       class:compact={self.compact}
       class:base={!self.invalid}/>
```

`base` is in the list and also under a directive, and the directive wins — an
invalid control loses `base` however the list was assembled.

> [!CAUTION]
> A directive puts the class name in the *attribute name*, where Tailwind and
> other CSS scanners do not look — the rule gets compiled out of your stylesheet.
> When a class must be discoverable by a scanner, use the map form, whose names
> are ordinary strings: `class={ "animate-pulse": self.busy }`.

## Attributes you cannot name

`{...expr}` splices a prepared run of attributes — the ones a component cannot
know the names of, like a computed `data-<controller>-target`:

```dmk
<input {...self.wiring} {...&self.data}>
```

`AttrSpread` is implemented for two things, and the difference is a security
boundary. **`&'static str`** is markup the author wrote, emitted verbatim; the
lifetime is what keeps a request-derived value out, since a string built from a
form field cannot be `'static`. **`[(K, V)]` and `Vec<(K, V)>`** are a map,
escaped on the way out, and that is where anything derived from state belongs.

helm needs none of this, and most components never will. [Attributes](/docs/attributes/)
and [Class lists](/docs/class-lists/) have the exhaustive rules.

## Back to the badge

The badge from chapter two can now carry its status in the markup instead of in
its text:

```dmk
<!-- src/status_badge.dmk -->
<span class=["badge", self.status.slug()] data-status={self.status.slug()}>{self.status}</span>
```

## Running it

Point `main` at the table:

```rust
use crate::model::{Service, Status};
use crate::service_table::ServiceTable;

fn main() {
    let services = vec![
        Service {
            name: "edge-router".into(),
            owner: "platform".into(),
            status: Status::Healthy,
            uptime_pct: 99.995,
            latency_ms: 42,
            version: "v2.14.0".into(),
        },
        Service {
            name: "checkout-api".into(),
            owner: "payments".into(),
            status: Status::Degraded,
            uptime_pct: 99.812,
            latency_ms: 380,
            version: "v5.1.2".into(),
        },
    ];

    let table = ServiceTable {
        services: &services,
        slo_target: 99.9,
    };
    println!("{}", table.render());
}
```

```sh
$ cargo run
…
    <tr>
      <td><div class="svc">edge-router</div><div class="owner">platform</div></td>
      <td>Healthy</td>
      <td>99.995%</td>
      <td>42 ms</td>
      <td class="ver">v2.14.0</td>
    </tr>
    <tr class="alt breach">
      <td><div class="svc">checkout-api</div><div class="owner">payments</div></td>
      <td>Degraded</td>
      <td>99.812%</td>
      <td data-slow>380 ms</td>
      <td class="ver">v5.1.2</td>
    </tr>
…
```

Three attributes are missing from that output, and all three are missing on
purpose: no `class` on the first row, no `data-slow` on the fast one. Nothing in
the template had to say so.

The status cell is still bare text, though — and there is a `StatusBadge` sitting
in the crate that renders exactly that cell. Putting it there is the next
chapter.
