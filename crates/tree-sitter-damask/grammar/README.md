# tree-sitter-damask

Tree-sitter grammar for [Damask](https://github.com/JWo1F/damask) `.dmk`
templates — compile-time components for Rust.

Damask templates are HTML with a `{ … }` tag syntax, JSX-style
`<Component/>` elements, and `<slot/>`s.

## Scope

The grammar owns the markup and the tag structure: elements, attributes, the
two attribute helpers, and the `{ }` tag family. It deliberately does **not**
parse the Rust inside a tag or the HTML it surrounds — those are exposed as
regions for injection, so the real Rust and HTML grammars highlight them.

```
attr="a {expr}"                      quoted, interpolating
attr={expr}                          expression
attr={@tokens(expr, "a", "n": cond)} a space-separated value
attr={@attrs(set, key: value)}       a run of `attr-*` attributes
class:name={cond}                    directive — `class` only
```

The helpers are written on any attribute, so nothing in the grammar is keyed to
a name: `class={@tokens(…)}` and `rel={@tokens(…)}` are the same rule, and so
are `data={@attrs(…)}` and `aria={@attrs(…)}`.

```
{ expr }                             expression, HTML-escaped
{@html expr}  {@render expr}         directives
{#if c}…{:else}…{/if}                conditional
{#for p in E}…{/for}                 loop
{#snippet name(params)}…{/snippet}   reusable fragment
{# … #}                              comment
```

## Use

Generate the parser at ABI 14 — Zed's bundled tree-sitter cannot compile ABI 15:

```sh
tree-sitter generate --abi 14
tree-sitter test
```

The generated sources under `src/` are committed, because editors that consume
this grammar compile `parser.c` directly rather than running the CLI.

## Where it lives, and who reads it

This directory is the grammar itself, not a copy of one. Three things read it,
and all three read *this* `src/parser.c`:

- **Zed**, through `editors/zed/extension.toml`, which pins a revision of this
  repository and points `path` at `crates/tree-sitter-damask/grammar`. Zed
  clones the revision, so a grammar change has to be pushed before the `rev`
  that adopts it.
- **The website**, through the `tree-sitter-damask` crate one directory up,
  whose `build.rs` compiles `src/parser.c` and whose `lib.rs` is the binding.
- **`tree-sitter test`**, over `test/corpus/`.

The highlight and injection queries are *not* here: they live with the Zed
extension, in `editors/zed/languages/damask/`, and the website reads them from
there — one set of queries decides what a `.dmk` looks like, in an editor or on
the page. A grammar change that renames a node is a query change too.

Regenerate after editing `grammar.js`:

```sh
tree-sitter generate --abi 14 && tree-sitter test
```

## License

MIT.
