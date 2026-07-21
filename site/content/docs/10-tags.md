+++
title = "Brace tags"
summary = "Printing, statements, raw output, control flow, imports and comments."
section = "Templates"
+++

Every construct the template language adds lives inside `{ … }`, and the contents
are a Rust block.

## `{ expr }` — print

Prints the value, HTML-escaped by the active renderer.

```dmk
<p>{self.name}</p>
```

Because the tag is a block, a statement runs and prints nothing, and the last
expression is what prints:

```dmk
{let count = self.rows.len()}
{2 + 3; 10}                  <!-- prints 10 -->
```

A literal brace has to be written as an expression: `{"{"}`.

## `{@html expr}` — print raw

No escaping. For content you produced yourself, or that is already escaped.

```dmk
<div class="prose">{@html self.body}</div>
```

## `{@render expr}` — render a snippet

Renders a snippet or a `Fragment`. **Not** a component — a component is called
with its tag.

```dmk
{#snippet chip(label)}<span class="chip">{label}</span>{/snippet}
{@render chip("all")}
```

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

The condition is a Rust expression, so `if let` works:

```dmk
{#if let Some(error) = &self.error}<p class="error">{error}</p>{/if}
```

## `{#each E as p}` / `{/each}`

`E` is any iterable expression — usually `&self.items`.

```dmk
{#each &self.items as item}<li>{item.label}</li>{/each}
```

A trailing identifier is the index; anything else is a whole pattern.

```dmk
{#each &self.chapters as chapter, i}<li>{i + 1}. {chapter.title}</li>{/each}
{#each &self.pairs as (key, value)}<dt>{key}</dt><dd>{value}</dd>{/each}
```

## `{#snippet name(params)}` / `{/snippet}`

Defines a reusable fragment. Snippets are `let` bindings, so a snippet must be
defined **before** it is rendered.

```dmk
{#snippet row(label, value)}<tr><th>{label}</th><td>{value}</td></tr>{/snippet}
```

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

## `{# … #}` — comment

Does not reach the output, unlike `<!-- … -->`.

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
