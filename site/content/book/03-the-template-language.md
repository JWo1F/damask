+++
title = "The template language"
summary = "Brace tags: printing, escaping, conditionals, loops, and the Rust inside them."
+++

A Damask template is HTML with **brace tags**. Everything the language adds lives
inside `{ … }`, and what is inside is a Rust block.

## Printing

```dmk
<p>{self.name}</p>
```

An expression prints its value, **HTML-escaped**. A statement or binding runs and
prints nothing:

```dmk
{let total = self.items.len()}
<p>{total} items</p>
```

Because the tag is a block, the last expression is what prints — `{2 + 3; 10}`
prints `10`.

A literal brace is written as an expression: `{"{"}`.

## Raw output

`{@html … }` prints without escaping:

```dmk
<div class="prose">{@html self.rendered_markdown}</div>
```

Use it for content you produced yourself or that is already escaped — a child
component's `.render()`, markdown you compiled at build time. Anything derived
from a request goes through `{ … }`. The asymmetry is the point: the dangerous
form is longer, named, and obvious in a diff.

## Conditionals

```dmk
{#if self.admin}
  <span class="badge">admin</span>
{:else if self.pending}
  <span class="badge badge--muted">awaiting review</span>
{:else}
  <span class="badge badge--plain">member</span>
{/if}
```

The condition is a Rust expression, so `if let` works too:

```dmk
{#if let Some(error) = &self.error}
  <p class="error">{error}</p>
{/if}
```

## Loops

```dmk
<ul>
  {#each &self.items as item}
    <li>{item.label}</li>
  {/each}
</ul>
```

The expression is anything iterable — usually `&self.items`, which is why the
binding is a reference. A trailing name is the index:

```dmk
{#each &self.chapters as chapter, position}
  <li><span class="num">{position + 1}</span> {chapter.title}</li>
{/each}
```

Anything else after the `as` is treated as a whole pattern, so destructuring
works:

```dmk
{#each &self.pairs as (key, value)}
  <dt>{key}</dt><dd>{value}</dd>
{/each}
```

## Imports

`{use …}` is an ordinary Rust `use`, and it is **scoped to the element that
encloses it**:

```dmk
<section>
  {use crate::ui::Badge}
  <Badge tone={self.tone}/>
</section>
<!-- Badge is not in scope here -->
```

Import anything — components, functions, enums. At the top of a template, before
any element, an import covers the whole file, which is where most of them go.

## Comments

`{# … #}` is a template comment. It does not reach the output, which is what
distinguishes it from `<!-- … -->`:

```dmk
{# The gap is on the header, not the caller: see CardHeader. #}
<div class="card">…</div>
```

## What does not work

**Control flow cannot appear in attribute position.** This is a parse error:

```dmk
<input {#if self.locked}disabled{/if}>
```

Attribute *names* are static — there is no `data-{key}=`. Express a conditional
attribute through its value instead, using a `bool` or an `Option`:

```dmk
<input disabled={self.locked} placeholder={self.hint.clone()}>
```

For a whole run of attributes whose names you do not know, there is `{...expr}`,
which the next chapter covers.
