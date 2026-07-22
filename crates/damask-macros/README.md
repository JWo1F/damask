# damask-macros

Procedural macros for [Damask](https://github.com/jwo1f/damask) — compile-time
components for Rust.

**You almost certainly want [`damask`](https://crates.io/crates/damask)
instead.** This crate exists only because Rust requires procedural macros to
live in their own crate: a `proc-macro` crate can export macros and nothing
else, so the `Component` derive cannot sit alongside the trait it implements.

```toml
[dependencies]
damask = "0.2"        # re-exports the derive; depend on this
```

## What it provides

The `Component` derive. Given a struct, it locates the sibling `.dmk` template
that shares the struct's snake-cased name, parses it with
[`damask-template`](https://crates.io/crates/damask-template), and generates a
`render_into` method that writes the markup directly — no runtime template
parsing, no build script, no configuration.

```rust,ignore
use damask::Component;

// greeting.rs — paired with greeting.dmk
#[derive(Component)]
pub struct Greeting {
    pub name: String,
}
```

Template resolution uses `Span::local_file`, which is why Rust 1.88 or newer is
required, and why editing a template triggers a rebuild without a `build.rs`.

## License

MIT.
