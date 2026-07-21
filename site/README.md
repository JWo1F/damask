# damask-site

The Damask website — a home page, a book, and a reference — generated from a
markdown database and rendered through Damask components into static HTML.

It doubles as the largest worked example in the repository: the whole site is
built out of `.dmk` components, styled with Tailwind pointed at the view tree,
exactly as the [book chapter on building a page](content/book/08-building-a-page.md)
describes.

## Building

```sh
./tools/build-site.sh          # compile the CSS, then render into site/dist
./tools/build-site.sh serve    # …and serve it on http://localhost:8080
```

`serve` runs a local static server so the clean URLs (`/book/slots/` →
`book/slots/index.html`) resolve the way they will once deployed — opening the
files directly does not.

Deploying under a subpath (a GitHub *project* page lives at `/<repo>/`):

```sh
BASE=/damask ./tools/build-site.sh
```

The generator can also be run on its own:

```sh
cargo run -p damask-site -- --out site/dist --base /damask
```

| Flag | Default | Meaning |
|---|---|---|
| `--content` | `site/content` | the markdown database |
| `--assets` | `site/assets` | copied verbatim into `dist/assets` |
| `--out` | `site/dist` | output directory, rebuilt from scratch each run |
| `--base` | *(root)* | URL prefix the site is served under |

## The content database

Everything the site says lives in `content/` as markdown with TOML front matter
fenced by `+++`.

```
content/
  home.md            the landing page (front matter is its structure)
  book.md            the book's title, lede, and intro prose
  book/
    01-why.md        chapters — the NN- prefix orders them and is dropped
    02-…             from the URL (01-why.md → /book/why/)
  docs.md            the reference's title and lede
  docs/
    10-tags.md       reference pages; `section = "…"` groups the sidebar
    11-…
```

A page's front matter:

```toml
+++
title = "Slots"
summary = "Placement, fallbacks, forwarding, and filling from Rust."
section = "Components"   # docs only — the sidebar group; omit for the first group
+++
```

Ordering comes from the **filename**, so the reading order is visible in a
directory listing and cannot contradict itself. Renaming a file's `NN-` prefix
reorders it without changing its URL.

Links between pages are written root-relative (`/docs/slots/#filling-from-rust`);
the base path is applied at build time, and **a link that does not resolve fails
the build** — see [`src/links.rs`](src/links.rs).

## How it is put together

| Path | What |
|---|---|
| `src/content.rs` | reads `content/` into the values the templates render |
| `src/markdown.rs` | markdown → HTML: heading anchors, build-time highlighting |
| `src/highlight.rs` | Syntect in classed mode; colours live in the stylesheet |
| `src/urls.rs` | the one place that knows the deploy's base path |
| `src/links.rs` | the internal link checker |
| `src/view/` | the components — `layouts/`, `ui/`, `pages/` |
| `ui/app.css` | Tailwind entrypoint, theme tokens, and the prose styles |
| `assets/` | `site.js`, the favicon — copied verbatim |
| `syntaxes/damask.sublime-syntax` | the highlighter's grammar for `.dmk` |

Code blocks are highlighted once, here, at build time — the reader downloads no
highlighter. The `.dmk` grammar is a highlighting approximation;
`crates/damask-template` remains the authority on what a template means.
