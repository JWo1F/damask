# damask-template

The template parser for [Damask](https://github.com/jwo1f/damask) — compile-time
components for Rust.

**Building a component? Use [`damask`](https://crates.io/crates/damask).** This
crate is for tooling that needs to read Damask templates itself — a formatter,
a linter, an editor plugin.

```toml
[dependencies]
damask-template = "0.2"
```

## What it does

Parses a `.dmk` template — HTML with a `{ … }` tag syntax, JSX-style
`<Component/>` elements, and `<slot/>`s — into a `Node` tree.

It is the single source of truth for template syntax, shared by two consumers
that must never disagree: the `Component` derive, which compiles templates at
build time, and `damask-lsp`, which analyses them as you type. A formatter that
parsed the syntax separately would drift from the compiler; this crate is how
that is avoided.

## What it deliberately does not do

It does not parse the Rust inside a tag or attribute. Rust is extracted as
text and left to `rustc` to type-check, which keeps the grammar small and means
the parser never needs to track Rust's evolution.

## Syntax

| Syntax | Meaning |
|--------|---------|
| `{ expr }` | Rust block — prints its value, HTML-escaped |
| `{@html expr}` | write `expr` raw, unescaped |
| `{@render expr}` | render a snippet or fragment |
| `{#if c}…{:else if c}…{:else}…{/if}` | conditional |
| `{#each E as p[, i]}…{/each}` | loop |
| `{#snippet name(params)}…{/snippet}` | define a reusable fragment |

Positions are tracked throughout, so consumers can map any node back to its
byte range in the source — the mechanism behind the language server's hover
and go-to-definition.

## License

MIT.
