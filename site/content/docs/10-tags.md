+++
title = "Brace tags"
summary = "Printing, statements, raw output, control flow, imports and comments."
section = "Templates"
+++

Every construct the template language adds lives inside `{ … }`, and the contents
are a Rust block. Braces balance, and string and char literals are stepped over
whole — so a `}` inside a Rust string does not end the tag.

## `{ expr }` — print

Prints the value, escaped by the active renderer.

```dmk
<p>{self.name}</p>
```

Because the tag is a block, a statement runs and prints nothing, and the last
expression is what prints:

```dmk
{let count = self.rows.len()}
{2 + 3; 10}                  <!-- prints 10 -->
```

A tag prints nothing when it ends in `;`, or when it opens with one of these
keywords:

`let` · `const` · `use` · `fn` · `static` · `type` · `struct` · `enum` ·
`trait` · `impl` · `mod`

An item declared this way is an ordinary Rust item in the enclosing scope, so a
helper `fn` may be defined in the template that uses it. A binding is `let`, and
the trailing `;` is optional — `{let x = 1}` and `{let x = 1;}` are the same.

A literal brace has to be written as an expression: `{"{"}`.

## `{@html expr}` — print raw

No escaping. For content you produced yourself, or that is already escaped.

```dmk
<div class="prose">{@html self.body}</div>
```

## `{@render expr}` — render content

Renders anything implementing `Render`: a snippet, a `Fragment`, a slot lookup,
an `Option` of any of them, or a component value built in Rust. The expression is
borrowed, not moved, and the content writes into the *same* renderer.

```dmk
{#snippet chip(label)}<span class="chip">{label}</span>{/snippet}
{@render chip("all")}
{@render slots.get("actions")}
```

A component written as a **tag** — `<Chip label="all"/>` — is the usual form;
`{@render}` is for content that is already a value.

## `{#if}` / `{:else if}` / `{:else}` / `{/if}`

```dmk
{#if self.admin}
  <span class="badge">admin</span>
{:else if self.pending}
  <span class="badge badge--muted">pending</span>
{:else}
  <span class="badge badge--plain">member</span>
{/if}
```

The condition is spliced into a Rust `if`, so `if let` works:

```dmk
{#if let Some(error) = &self.error}<p class="error">{error}</p>{/if}
```

## `{#each E as p}` / `{/each}`

`E` is any `IntoIterator` expression — usually `&self.items`. The tag is split on
the first ` as `, and both halves must be non-empty.

```dmk
{#each &self.items as item}<li>{item.label}</li>{/each}
```

A trailing `, ident` is the index; anything else is a whole pattern.

```dmk
{#each &self.chapters as chapter, i}<li>{i + 1}. {chapter.title}</li>{/each}
{#each &self.pairs as (key, value)}<dt>{key}</dt><dd>{value}</dd>{/each}
```

| Form | Lowers to |
|---|---|
| `{#each E as p}` | `for p in E { … }` |
| `{#each E as p, i}` | `for (i, p) in (E).into_iter().enumerate() { … }` |

## `{#snippet name(params)}` / `{/snippet}`

Defines a reusable fragment. The name is everything before the first `(`, the
parameters everything up to the last `)`; the parameter list may be empty.
Snippets are `let` bindings, so a snippet must be defined **before** it is
rendered and goes out of scope with the element it was defined in.

```dmk
{#snippet row(label, value)}<tr><th>{label}</th><td>{value}</td></tr>{/snippet}
```

See [Snippets and fragments](/docs/snippets/) for what the two forms lower to.

## `{use path}`

An ordinary Rust `use`, **scoped to the enclosing element**. Import components,
functions, enums, anything.

```dmk
<section>
  {use crate::ui::{Badge, Dot}}
  <Badge tone={self.tone}/>
</section>
<!-- Badge is out of scope here -->
```

At the top of a template, before any element, the import covers the whole file.
`Component` and `Render` are already in scope in every template, so
`child.render()` needs no import.

## `{# … #}` — comment

Does not reach the output, unlike `<!-- … -->`. The whitespace after `{#` is what
tells it from a block tag, so `{#if}` is never mistaken for one. Braces and tags
inside a comment are prose, not structure, and the closing `#}` is required.

```dmk
{# The bottom margin belongs to the header, not the caller. #}
```

## Restrictions

Control flow **cannot appear in attribute position**:

```dmk
<input {#if self.locked}disabled{/if}>   <!-- parse error -->
```

Attribute names are static — there is no `data-{key}=`. Express a conditional
attribute through a `bool` or `Option` value, or build the whole run in Rust and
splice it with [`{...expr}`](/docs/attributes/#spreading).

An empty tag (`{}`), an empty expression in `{@html}`, `{@render}`, `{#if}` or an
attribute value, an unknown directive (`{@foo}`), an unknown block (`{#foo}`) and
an unknown clause (`{:foo}`) are all errors at build time, reported against the
template.
