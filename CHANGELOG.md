# Changelog

All notable changes to Damask are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/), and the project adheres to
[Semantic Versioning](https://semver.org/).

## [0.11.0] - 2026-09-04

### Changed

- **One syntax for every attribute: `{@tokens(…)}` and `{@attrs(…)}`.** `class`
  and `data` no longer have forms of their own. What a value does beyond
  `attr={expr}` is now asked for by a helper — the same two on any attribute —
  so the compiler, the grammar, the language server and the reader all stop
  keeping a list of which names are special:

  ```html
  <div class={@tokens(self.extra, "base", "is-open": self.open)}
       rel={@tokens("noopener", external: self.away)}
       data={@attrs(self.hooks(), controller: "modal", index: self.i)}
       aria={@attrs(label: self.title)}>
  ```

  `@tokens` builds one space-separated value: positional entries are names, lists
  of them or `Option`s, a `name: cond` entry is there while its condition holds,
  names dedupe and keep first-mention order, and an empty result omits the
  attribute. `@attrs` expands one set into a run of `<name>-*` attributes, where
  the prefix is the attribute it was written on — which is what makes `aria`,
  `hx` and anything else shaped that way work without the compiler having heard
  of them.

  The migration is mechanical:

  | Before | Now |
  |---|---|
  | `class=[a, "b", { "c": cond }]` | `class={@tokens(a, "b", "c": cond)}` |
  | `class={ "c": cond }` | `class={@tokens("c": cond)}` |
  | `data={self.hooks()}` | `data={@attrs(self.hooks())}` |
  | `data=[base, { "open": x }]` | `data={@attrs(base, open: x)}` |
  | `data={ "controller": "modal" }` | `data={@attrs(controller: "modal")}` |

  Both old forms are refused with an error naming the replacement, so a template
  that has not been migrated says so rather than compiling to something else.
  `class:name={cond}` is unchanged and is now the only attribute whose *name*
  carries meaning. `tag!` takes the helpers in the same spelling —
  `tag!(div, class: @tokens("a", on: flag))`.

  Two things fall out of the change rather than being designed into it.
  `data="movie.swf"` and `data={self.url}` are the ordinary attribute they look
  like, on `<object>` as anywhere else, so the rule about writing a dynamic one
  as `data="{self.url}"` is gone. And because `@tokens` builds a value, it works
  on a component prop — `<Button class={@tokens("w-full", "is-busy": busy)}/>` —
  which a class list never could.

- **`ClassList` is `TokenList`, and the data traits are the attribute traits.**
  A list that any attribute may hold is not a class list, so `ClassList` and
  `ClassItem` are now `TokenList` and `TokenItem`, and `TokenItem` gained impls
  for slices, arrays and `Vec`, so a `Vec<String>` of names is one entry.

  `DataSet`, `DataItem` and `DataValue` are **removed**. A group is name/value
  pairs waiting to be written, which is exactly what `Attrs` already held for
  `#[prop(rest)]`, so `@attrs` builds an `Attrs` through `AttrSet` and
  `IntoAttrValue` — one set of traits instead of two parallel ones, with
  `Attrs::write_attrs_prefixed` supplying the prefix. `AttrSet` gained the
  `HashMap` and `BTreeMap` impls `DataItem` had (`HashMap` still visited in key
  order, so output stays stable), and `IntoAttrValue` gained `char`.

- **A key written in a template is checked when it compiles.** An `@attrs` key
  becomes half of an attribute *name*, where escaping is no defence, so a key
  that could end the name and begin another is now a build error instead of an
  entry silently dropped at render time. A key is a bare identifier or a string,
  taken verbatim either way — `user_id` is still `data-user_id`.

- **The Tree-sitter grammar moved into this repository**, to
  `crates/tree-sitter-damask/grammar`, beside the Rust binding the website reads
  it through. Zed pins a revision of this repository and points its `path` at
  that directory, so the grammar, the queries that colour it and the compiler
  that agrees with both now move in one commit instead of two repositories kept
  in step by a vendored copy. The `class_*` rules are gone; a helper is one
  `helper` node with `helper_key`, `helper_expr` and `helper_value` inside it,
  and the queries follow.

### Added

- **The language server knows the helpers.** `@` in an attribute value completes
  to `@tokens(…)` and `@attrs(…)` — as calls, with the cursor left inside the
  parentheses — and hovering either explains what it builds, what its entries
  may be and how its values decide whether they appear. Neither is Rust, so
  rust-analyzer could only ever have answered that position with the members of
  `self`.

## [0.10.2] - 2026-09-03

### Fixed

- **A `#[prop(rest)]` fallback no longer shadows a template's own method of the
  same name.** An attribute on a component tag brings a one-method trait of that
  name into the render function's scope, and it was blanket-implemented — so a
  `class="field"` written on any component made `{row.class()}` elsewhere in the
  same template resolve to *that* rather than to `Row::class`, and report it as
  errors about a component nobody wrote. A setter takes `self` by value, so the
  fallback does too, and a by-value candidate is picked ahead of an autoref one:
  an inherent `fn class(&self)` lost.

  The bound moved from the methods (`where Self: Rest`) onto the impl
  (`impl<T: Rest>`), which leaves the fallback inapplicable to anything that is
  not a props builder — the set it was written for — so method resolution never
  considers it for a value of the template's own. The refusal a component
  without a bag gets is unchanged in wording and now an `E0599`;
  `examples/attributes` has the case as `Row`, and both compile-fail snapshots
  are in `tests/ui`.

  What made this hard to see is that a **borrowed** receiver was never affected:
  `{#for row in &rows}` reached the inherent method, so the same call compiled or
  not depending on whether the template held the value or a reference to it.


## [0.10.1] - 2026-09-03

### Fixed

- **A `#[prop(rest)]` bag seeded by `Default` is no longer thrown away by a call
  site.** With `#[component(default)]` the builder starts from the struct's
  `Default` and overwrites each prop the call site set — which is right for a
  prop and wrong for a bag, since a bag is a collection a call site *adds* to.
  A component whose `Default` put `class="btn"` in it lost that the moment any
  attribute was written at the call site, silently. The two are now merged, with
  a name written in both taking the call site's value and keeping the default's
  position — `Attrs`' own rule, and what overriding a default means for every
  other prop. `examples/attributes` has the case as `Seeded`.

  The non-defaulted path was never affected: it builds the struct field by
  field, so there is no default bag there to lose.

## [0.10.0] - 2026-09-03

### Added

- **A component can carry the attributes it does not name.** One field marked
  `#[prop(rest)]` collects every attribute a call site wrote that is not a prop,
  and the component's template decides where they land by spreading it:

  ```rust
  #[derive(Component, Default)]
  #[component(default)]
  pub struct Hidden {
      #[prop(rest)]
      pub attrs: Attrs,
  }
  ```

  ```html
  <input type="hidden" name={self.name()} {...self.attrs}>   <!-- hidden.dmk -->

  <Hidden data-cover-target="input" aria-hidden="true" autofocus/>
  ```

  Which of a tag's attributes are props cannot be answered by the template
  lowering — it runs in a different crate from the component and cannot see its
  fields — so it does not answer it. It emits the setter call it always emitted
  *and* a fallback trait of the same name that routes to the bag, and lets
  **method resolution** choose: an inherent setter, which is a declared prop,
  wins, and only a name the builder has no setter for falls through. A name that
  could not be a method at all — everything hyphenated, and every Rust keyword —
  goes to the bag on sight, which is why `<TextInput type="email"/>` now works
  even though no field can be called `type`.

  The bag is opt-in, and that is the point: a component without a
  `#[prop(rest)]` field does not implement `props::Rest`, so an attribute it does
  not declare is a build failure naming the attribute. A component that accepted
  everything could not tell a passed-through attribute from a misspelled prop.

  `Attrs` holds name/value pairs, keyed the way `DataSet` is — first position,
  last value — with `get`, `contains` and `iter` so a component can read what it
  was given rather than only pass it on. `IntoAttrValue` decides what a value
  means, and covers the set `Attr` does: a `bool` is a bare attribute or none,
  an `Option` is nothing when it is `None`.

- **`{...expr}` on a component tag**, which was an error. It is a whole set for
  the bag, folded in where it was written — the way a wrapper forwards its own
  attributes to the component inside it. `AttrSet` is what a set is: an `Attrs`,
  a list of pairs, or an `Option` of either.

- **`examples/attributes`**, which is all of the above running, with the two
  compile-fail cases as `trybuild` snapshots.

### Changed

- **A spread no longer writes an attribute the tag already writes itself.**
  `<input type="text" {...self.extra}>` with a `type` in `extra` emits one
  `type`, from the tag, rather than two — a duplicate attribute is not valid
  HTML and browsers resolve it by a rule nobody writing the template was
  thinking about. The skipped names are the literal attribute names in that tag,
  decided when the template compiles, so it costs a scan of a list that is
  usually empty. `AttrSpread::write_attrs_except` is the new method; its default
  ignores the filter, which is the honest answer for a `&'static str` spread that
  has no names to compare.

## [0.9.0] - 2026-09-03

### Changed

- **A skippable prop's setter takes the value, not the `Some` around it.** A
  prop typed `Option<T>` has already said that leaving it out is allowed, and a
  call site saying it a second time added nothing:

  ```html
  <Notice detail={self.detail} dismissible/>
  <Card rows={4} of={form}/>
  ```

  Those used to be `detail={Some(..)}`, `rows={Some(4)}` and `of={Some(form)}`.
  The setter for a skippable prop is now `impl Into<Option<T>>`, so both
  spellings compile — `Option<T>` still reaches `Option<T>` reflexively — and
  the noisier one is no longer the only one. A **required** prop's setter is
  unchanged and still takes its type exactly, which is what keeps coercion
  (`&Vec<T>` to a `&[T]` prop), integer inference, and the pinning of a generic
  component's parameter.

  What it costs is inference where the value alone does not say what it is:
  `class={None}` no longer knows which `None`, and is written `class={None::<String>}`
  or, better, left out.

- **A quoted attribute reaches its prop through a setter of its own.**
  `title="Deploy finished"` now lowers to `__damask_literal_title("…")` and
  `detail="row {self.n}"` to `__damask_text_detail(format!(…))`, both generated
  per prop beside the setter they belong to. `props::literal` infers what to
  build from where it is going, and an `impl Into<…>` parameter is not one
  destination but a set of them — so the conversion moved to where the prop's
  type is written down. `props::FromInterpolated` is the new half of that, and
  the mirror of `FromLiteral` for a value that arrives as a `String`.

  This is internal machinery: templates are unchanged, and a `#[doc(hidden)]`
  setter is not something a call site names. It is listed because the generated
  surface is what a `Component` derive *is*.

## [0.8.1] - 2026-09-01

### Fixed

- **`damask-lsp` is published again.** The language server stopped at 0.3.2
  while the workspace went to 0.8.0, and it is distributed only through the
  registry — there is no prebuilt binary, and the Zed extension, both READMEs
  and the tooling docs all say `cargo install damask-lsp`. So the server a
  reader installed predated both `data` attributes and async templates, and
  reported every `.await` in a template as a diagnostic on code the derive
  compiles. This release carries it to the registry with the rest.

- **The site's landing page reads its version from `Cargo.toml`.** The hero's
  dependency snippet and the woven "rendered" panel both had the number typed
  into `site/content/home.md`, where it stayed at 0.5 while the workspace went
  to 0.8. `home.md` now writes `{{version}}` and the generator substitutes the
  workspace version — `damask-site` inherits `version.workspace = true`, so its
  `CARGO_PKG_VERSION` is the number the release process already bumps and tags,
  and the page cannot fall behind it again.

### Changed

- **The landing page's hero shows the call site.** The woven example was a
  struct, a template and the markup they produce, with nothing on the page
  saying what was rendered — so the card's body, which comes from the caller,
  and its footer, which comes from a slot's fallback, both read as things the
  template invented. A fourth panel, `page.dmk`, holds the `<Card …>` that
  produced the output panel, and `compiles to` moved to sit between those two,
  which is the pair it is true of.

## [0.8.0] - 2026-09-01

### Added

- **`<Card await/>`, the marker that makes an async-only child usable.** A
  template could not call a component whose own template awaits unless the
  enclosing template happened to await something else — the scan that decides
  which render path to emit sees markup, and whether `<Card/>` is asynchronous
  is a property of a *type*, on the other side of a macro-expansion boundary.
  So the child was a compile error at the call site with nothing to do about it
  but add a dummy `.await`.

  `await` on a component element says what only the author can know. It is not
  a prop — `await` is a keyword and could not name one — and it makes the
  enclosing template asynchronous, exactly as a real `.await` in it would. On an
  HTML element it is refused, rather than written out as an attribute nobody
  meant.

## [0.7.0] - 2026-09-01

### Added

- **A slot fill may `.await`.** Markup written between a component's tags can
  now suspend, so an awaiting component can be the child of one that is not:

  ```html
  <Card>
    <Slow/>
  </Card>
  ```

  Previously this was refused at lowering — "`.await` is not supported inside
  `<Card>`'s default slot content" — because a fill travelled to the callee as
  a plain `&dyn Render` and there was no async-shaped fill to hand across that
  boundary. Now there is one, and the callee is unchanged: it renders `<slot/>`
  exactly as before, and only the async path can produce the markup.

  Each fill is judged on its own body rather than on the enclosing template, so
  a fill that suspends nowhere stays a plain `Fragment` and costs no boxed
  future even when something else in the same template awaits.

- **`Fill`**, the content of one slot and whether producing it suspends, with
  `Slot::new_async` for building the second kind.

### Changed

- **`Slots::get` and `Slots::get_default` return `Option<Fill<'a>>`** rather
  than `Option<&dyn Render + Sync>`. `{@render slots.get("footer")}` still
  works — `Fill` is `Render` — and now **panics on a fill that awaits**, naming
  `Slots::render_async` and the `<slot name="footer"/>` form that has an async
  path. The panic is unreachable from a template, since a fill that awaits
  makes its whole enclosing template await; it is what a hand-built `Slots`
  gets instead of silently rendering nothing.

  Code that only writes `<slot/>` and passes fills through the macro needs no
  change. Code calling `Slots::get` and holding the result as a
  `&dyn Render` does.

## [0.6.0] - 2026-08-31

### Added

- **`Trusted`, and the `tag!` macro that builds one.** Markup that is already
  safe to write, and a way to build an element in Rust rather than in a
  template — for a helper in a service, a fragment a handler assembles, or a
  `<style>` element whose stylesheet is full of the `{` a template reserves.

  ```rust
  tag!(div #summary, class: ["card", urgent.then_some("is-urgent")], {
      (tag!(span, "Total"), tag!(b, user_supplied))
  })
  ```

  The element comes first, then `name: value` attributes in any order, then the
  content with no name. `class:` and `data:` take the forms a template takes
  and go through the same `ClassList` and `DataSet`, so `data: { open: true }`
  writes a bare `data-open` here exactly as `data={…}` does there. A `&str`
  child is escaped and a `Trusted` one is spliced, which is what makes
  `tag!(p, user_name)` safe and `tag!(p, tag!(b, "x"))` markup without either
  saying so. A void element takes no content, and saying otherwise is a
  compile error.

  An id may be written in the head, but needs a space — `tag!(div #main)` —
  because `div#main` is not Rust: `ident#` has been a reserved prefix since
  edition 2021, so those tokens never reach a macro. `id: "main"` is the
  ordinary way to write it.

- **Documentation: async templates on the site.** The feature shipped in 0.4.0
  with nothing about it outside the changelog and the agent skill. Now there is
  a book chapter (*The live fleet*, chapter eight) that arrives at `.await` from
  helm needing a store rather than a fixture, and a reference page
  (*Async templates*) covering the detection rule, the trait pair and its
  blanket impls, the `Send`/`Sync` bounds and what each one is for, and the two
  places `.await` cannot go. Every compiler error quoted in either was taken
  from a real build, and every example compiled before it was written down.

  The neighbouring pages gained what belongs on them rather than repeating it:
  `AsyncComponent`/`AsyncRender`/`RenderFuture` on Traits, the async branch of
  the expansion on The Component derive, `Slots::render_async` and a fill's
  `Sync` on Slots, `fragment_async` and the parameterized-snippet limit on
  Snippets, `Renderer: Send` on Renderers. The README gained an *Async
  templates* section of its own, since it had no mention of the feature at all.

- **The landing page's feature list is set as a specimen sheet.** It gained two
  entries — async, and the renderer seam, which the page had never made a claim
  about — and six equal blocks in a bare two-column grid had stopped being a
  composition. Now each entry hangs a mono numeral in its own column and sits
  under a hairline rule, the section says its name in the mono eyebrow the hero
  already uses, and the feature titles are `h3`s under that `h2` rather than a
  flat run of `h2`s. The rules are also what make the layout indifferent to the
  number of features: a last row of one reads as the end of a table rather than
  as an orphan, which the old grid could not do.

### Changed

- **`{ … }` writes markup through and escapes everything else.** It used to
  escape whatever it was given, so a helper returning markup could only be
  spliced by `{@html …}` — a tag that splices *anything*, and therefore cannot
  tell "markup I built" from "a string I have decided to trust". Now the value
  decides: `Trusted` writes through, everything else escapes exactly as before.
  No template changes, and `{@html …}` is unaffected.

  This works because `Trusted` deliberately implements **neither `Display` nor
  `ToString`**. A blanket impl over `Display` may sit beside a specific impl
  for `Trusted` only because `Trusted` is local to this crate and `Display` is
  `std`'s, so rustc can see no other crate is allowed to write the impl that
  would make the two overlap. Adding `impl Display for Trusted` would break
  the `Value` trait, which is why markup is read back with `Trusted::as_str`
  rather than through `format!`.

- **The website highlights code with Tree-sitter.** Syntect and the
  hand-written `damask.sublime-syntax` beside it are gone; the site now parses
  each fenced block with the same Tree-sitter grammar the editors use
  (`crates/tree-sitter-damask` vendors the generated parser at the revision
  `editors/zed/extension.toml` pins) and colours it with the Zed extension's own
  `highlights.scm` and `injections.scm`, read straight out of `editors/zed/`.
  A `.dmk` snippet in the documentation is therefore coloured by the rules that
  colour the file it was taken from, and there is one place to fix it when it
  looks wrong rather than two that drift.

  The other fences moved with it — Rust, HTML, CSS, TOML and shell are upstream
  grammars now. Two things worth naming:

  - The `json`, `js`, `yaml`, `md` and `markdown` fence aliases no longer
    highlight. Nothing in the content used them; each would mean carrying
    another grammar, and adding one back is a dependency and a table row.
  - Every `.dmk` block on the site now parses, bar the two that are *meant*
    to: *Brace tags* and *The template language* both show `<input {#if …}>`
    to say that control flow cannot appear in attribute position, and the
    grammar agreeing with the compiler is the point. Getting there took a
    grammar fix and a documentation fix, both below.

  Token classes were renamed with the vocabulary they come from: `.tok-punct`,
  `.tok-attr`, `.tok-number`, `.tok-brace` and `.tok-builtin` replace the
  Syntect scope atoms `.tok-punctuation`, `.tok-attribute-name`, `.tok-numeric`,
  `.tok-damask.tok-embedded` and `.tok-language`. The palette is unchanged —
  `site/src/highlight.rs` maps every capture name onto it, and is the only file
  to edit to recolour a kind of token.

- **`dev-setup.sh` refuses to delete a grammar clone that has work in it.** Zed
  keeps its own clone of the grammar at `editors/zed/grammars/damask/`, and the
  script cleared it unconditionally so Zed would re-clone at the pinned
  revision. That directory is git-ignored, so nothing ever shows what is in it,
  and it is an ordinary clone of the grammar — which makes it an easy place to
  edit the grammar by mistake and a silent place to lose the edit.

  It is not hypothetical. A correction to the grammar's README and test corpus,
  spelling the loop `{#for p in E}` instead of the `{#each E as p}` it stopped
  being, had been sitting there uncommitted — found only because a checkout of
  the new revision refused to run over it, which is also what broke installing
  the extension. It is upstream now.

  The script checks three ways work can exist only there — uncommitted or
  untracked files, stashes, and commits on no remote — and on any of them
  prints what it found and exits rather than deleting. `DAMASK_FORCE_GRAMMAR=1`
  deletes anyway. It still clears a directory that is not its own repository,
  since a half-finished clone has nothing to protect.

- **The Zed extension is 0.2.0.** Its queries and its pinned grammar both moved
  under it — the injection fixes, `data`'s attribute forms, the prefix that no
  longer eats a name — and the manifest still said `0.1.0`. Zed decides whether
  an installed extension is stale by comparing that number, so every one of
  those fixes would have sat on `master` reaching nobody. `CLAUDE.md` now
  carries the rule, since the only way to notice the omission is to already
  know about it.

- **The landing page's first example shows a named slot, and stops opening on
  an alarm.** The hero says *components with real slots* and the example under
  it filled one anonymous `<slot/>` — the named slots, the fallback content and
  the forwarding the lede promises were nowhere in the first thing a reader
  sees. `card.dmk` now declares a `meta` slot with fallback beside the default
  one, and the rendered panel shows that fallback standing in, which is the
  whole mechanism in three lines.

  Its subject changed with it. The page used to open on a disk filling up —
  *Disk almost full*, *3% left on /dev/sda1* — which is a strange first
  impression for a library that has nothing to do with monitoring, and a bleak
  one to lead a landing page with. It is a release note now. The two panels
  also came out the same height, which the compiles-to rule between them was
  always asking for.

### Fixed

- **`data` takes the `class` forms, and a prefix no longer eats a name.**
  Upstream in [tree-sitter-damask][tsd], so the editor gains both too. The
  grammar knew `class=[…]` and `class={…}` but not `data=`, though the compiler
  parses them into the same value — so every snippet showing a `data` list or
  map fell out of the tree below the attribute.

  Adding `data` to the prefix token alone would have made it worse. The token
  carried `prec(1)`, and explicit token precedence is settled *before* match
  length, so the prefix beat a longer attribute name: `classy` and `class-foo`
  already parsed as `class` followed by an error, and `data` would have done the
  same to every `data-*` attribute there is. The precedence moved to the
  directive form, whose token now carries its own colon — `class:` — which is
  narrow enough that it cannot bite a name, and long enough to win against an
  `attribute_name` that accepts colons. The bare prefix keeps no precedence at
  all and wins on being a literal rather than a pattern.

  [tsd]: https://github.com/JWo1F/tree-sitter-damask

- **Two documentation snippets did not compile.** *Class lists* and *Data
  attributes* each opened with a multi-line element whose attributes were
  labelled by trailing `<!-- … -->` comments — and a comment cannot sit inside
  an open tag, in Damask or in HTML. `damask_template::parse` rejects both with
  *expected an attribute name*. The labels moved into the sentence above each
  block, which the headings below already spell out in full.

- **Rust inside a `class` value had no colour, in the editor as well as on the
  site.** `injections.scm` captured `class_expr`, `class_code` and
  `class_condition`, each of which is wholly covered by the `code` node it
  wraps — and an injection defaults to the bytes a node owns *itself*, once its
  children are removed. That left an empty region to inject into, so
  `class=[self.tone.skin()]` and `class={ "on": self.busy }` came back plain.
  The same default was cutting string literals out of an injected tag, which is
  why `{ "hi".len() }` highlighted everything except the `"hi"`. Every pattern
  now sets `injection.include-children`, which is also what keeps a nested brace
  group part of the expression around it rather than a hole in it.

- **A trailing comment's own words were painted as attribute names.** The old
  Sublime syntax popped its attribute context on the first `>` it met, and the
  `-->` ending a comment is one, so on *Class lists* the words inside
  `<!-- quoted, interpolating -->` came out coloured as if they were attributes
  and the following lines lost their colouring. The grammar reads the comment
  as a comment and recovers the lines below it, as far as it can — see above
  for the part of that block it still cannot parse.

- **Documentation: the install snippets said `0.2`.** Five of them, across the
  README, the landing page, two book chapters and the renderers reference — all
  now `0.5`. Pasting `0.2` got you a version with no async in it at all.

- **The site's header overflowed every page sideways on a phone.** The row wants
  425px of wordmark, sections, search and two icon buttons; a 375px screen
  offers 335px, so every page on the site scrolled horizontally — including ones
  with nothing wide in them. The wordmark is now `sr-only` below `sm`, which is
  the 90px that did it: `hidden` would have taken the home link's only text and
  left it with no accessible name, while `sr-only` keeps it in the accessibility
  tree and out of the layout. Gaps and the section links' padding tighten a
  little below `sm` alongside it. Below 360px the GitHub icon steps out too — it
  is the one control up there that the footer also carries on every page.

## [0.5.0] - 2026-08-30

### Added

- **`damask_template::template_awaits`**, and template resolution moved into
  `damask-template` alongside it (`resolve`, `Resolved`, `component_basename` —
  it was private to `damask-macros`). A framework wrapping
  `#[derive(Component)]` emits code of its own beside the derive's, and what a
  component *is* now differs between the two cases: an awaiting one implements
  `AsyncComponent` and so has no `Component` for that code to hang a response
  impl on. A wrapper macro has no type information to tell them apart, and
  rustc rejects an impl whose `where Self: Component` is trivially false — so
  the question has to be answerable from the struct, which is what this does.

### Changed

- **Breaking: an async render is `Send`, so it can be awaited by a server.**
  `RenderFuture` was `Pin<Box<dyn Future + 'a>>`, which meant a future awaiting
  one was not `Send` — and a work-stealing executor cannot drive a future that
  is not. The practical consequence was that a template written with `.await`
  in it could not be rendered from a request handler at all, which is most of
  what the feature is for. Three bounds make it attainable, each on the thing
  the future actually holds across an `.await`:

  - `Renderer: Send`, for the `&mut dyn Renderer` a render writes through.
  - `Slot`'s content is `&(dyn Render + Sync)` rather than `&dyn Render`, for
    the `Slots` an async template carries past each of its awaits. Deliberately
    *not* a `Sync` supertrait on `Render`: a component that neither fills a slot
    nor awaits anything — a generic one especially — is unaffected.
  - `AsyncRender: Sync`, for the component's own `&self`. It falls out
    automatically for a plain struct of data; a generic component that wants to
    await names `Sync` on the parameters it holds.

  A hand-written `Renderer` over a non-`Send` backing store, or a slot filled
  with non-`Sync` content, is what this can break. Neither is affected by
  anything the derive emits.

### Fixed

- **Documentation: `#[component(crate = …)]` on the site.** The derive page said
  `default` was the only option `#[component]` takes, which stopped being true in
  0.4.0; it now documents `crate` alongside it, as the README does.

## [0.4.0] - 2026-08-30

### Added

- **Async templates.** Write `.await` anywhere a `.dmk` holds Rust — `{ … }`,
  an `{#if}`/`{#for}` condition or iterable, an attribute value — and the
  derive emits an async render path automatically; no attribute to add, and a
  template with no `.await` anywhere stays exactly as sync as before, at no
  extra cost. An async component implements the new `AsyncComponent`/
  `AsyncRender` traits instead of `Component`/`Render`, and renders with
  `.render_async().await`. Every sync `Render` gets `AsyncRender` for free
  (blanket impls in `damask`), so an async template can embed a plain sync
  child at no real cost; the reverse — a sync template embedding an
  async-only child — is a compile error, since there is no sync fallback that
  wouldn't mean blocking on a future inside the caller's own executor.

  Two spots don't support `.await`: a component's slot fill (the children
  between `<Comp>…</Comp>`, which travel to the callee as a plain
  `&dyn Render`), and a `{#snippet}` that takes parameters (its closure has to
  stay callable more than once, which an `async move` capturing a
  non-`Copy` parameter can't guarantee). Both have a one-line rewrite — see
  the "Async templates" section of the component-authoring skill.

  New in `damask`: `AsyncRender`, `AsyncComponent`, `RenderFuture`,
  `AsyncFragment`/`fragment_async`, and `Slots::render_async`. New in
  `damask_template`: `needs_async`.

- **`#[component(crate = ::some::path)]` names the path generated code reaches
  Damask through.** For a framework that re-exports Damask rather than having
  its users depend on it: the default `::damask` resolves against the extern
  prelude and nowhere else, so without this every application would have to
  name `damask` in its own `Cargo.toml` just to write a template. The lowering
  takes the path too — `damask_template::lower_with` and `lower_mapped_with`,
  alongside the unchanged `lower` and `lower_mapped`.

- **`data` expands one value into a run of `data-*` attributes.** The second
  attribute with forms of its own, and the Rails `data: { … }` equivalent:
  `data={self.hooks()}` takes anything implementing the new `DataItem` trait —
  pair lists, `HashMap`, `BTreeMap`, `DataSet`, an `Option` of any of them, or
  your own type — and `data=[…]` merges several sources, with a later mention of
  a key overriding an earlier one. `data={ "controller": "modal" }` is the
  inline map. Values implement `DataValue`, which mirrors `Attr` one level down:
  a `bool` renders a bare `data-open` or nothing, an `Option` renders nothing
  when `None`. Keys are verbatim, not dasherized. A `HashMap` is visited in key
  order so the same state always renders the same attribute order.

  A quoted `data="…"` is untouched and stays an ordinary attribute, which is
  what leaves `<object data="movie.swf">` alone — only `data={…}` and `data=[…]`
  expand. Longhand `data-*` attributes are likewise left on the `Attr` path and
  are not merged into the set.

- **`is_attr_name_safe`**, the check both the `data` keys and the key/value
  `AttrSpread` apply to a name before writing it.

### Fixed

- **A key/value `{...expr}` spread could inject an attribute.** `AttrSpread for
  [(K, V)]` escaped the name, but escaping cannot make a name safe: a key
  holding a space or an `=` ends the name and begins a second attribute, so a
  key derived from state could smuggle in an `onclick`. Such a pair is now
  dropped, and trips a `debug_assert` in a debug build. Names written in a
  template were never affected — the parser does not accept one.

## [0.3.2] - 2026-07-23

### Added

- **Language server: go-to-definition for component attributes and slot fills.**
  Cmd/Ctrl-click on a component attribute now jumps to the struct field it sets,
  and on a `slot="…"` fill to the `<slot>` declaration in the target component's
  template — the same reason hover needed native support: both lower to
  generated setters (or, for slots, to nothing) that rust-analyzer cannot follow.
  Component *names* already resolved through rust-analyzer and still do.

## [0.3.1] - 2026-07-23

### Added

- **Language server: component, attribute, and slot intelligence.** Hovering a
  component attribute now shows the field's type, whether it may be omitted
  (`Option<_>` or `#[component(default)]`), and its doc comment — where before
  rust-analyzer saw only the generated builder setter it lowers to. Slot fills
  autocomplete: typing `slot="…"` on a child of a component offers that
  component's declared slot names, and `slot` itself is offered as an attribute.
  Hover explains a `<slot>` declaration and a `slot="…"` fill (flagging a name
  the component does not declare). Component and prop completions now carry doc
  comments, and props are marked optional in the list.

### Changed

- **Language server: much lower memory use.** A multi-crate workspace now runs a
  single rust-analyzer, rooted at the workspace, rather than one per member crate
  — the main cause of the server's footprint growing several-fold in a workspace.
  A closed template's overlay and HTML skeleton are released rather than kept for
  the session.

## [0.3.0] - 2026-07-23

### Changed

- **Breaking.** Loops are now written as a Rust `for`: `{#for pat in E}…{/for}`
  replaces `{#each E as p}…{/each}`. The header is Rust verbatim — `pat` is any
  pattern and `E` any iterable — so there is no bespoke loop syntax to learn and
  the whole header injects and highlights as the Rust it is. The old `, i` index
  shortcut is gone in favour of Rust's own `.enumerate()`:

  | Before | After |
  |---|---|
  | `{#each &self.items as item}…{/each}` | `{#for item in &self.items}…{/for}` |
  | `{#each &self.xs as x, i}…{/each}` | `{#for (i, x) in self.xs.iter().enumerate()}…{/for}` |
  | `{#each E as (k, v)}…{/each}` | `{#for (k, v) in E}…{/for}` |

  `{#each}` is no longer recognized and is a parse error.

## [0.2.0] - 2026-07-22

### Added

- A template can ask about its own slots. The caller's fills are in scope for
  every `{ … }` tag as `slots`, so `{#if slots.has("actions")}<footer>…</footer>{/if}`
  drops a wrapper the caller gave nothing to render inside — the case a `<slot>`
  fallback cannot express, standing in as it does for the content rather than the
  markup around it. `Slots` gains `has`, `has_default` and `get_default` to go
  with `get`, and `{@render slots.get("actions")}` places a fill by name.
- `Render` is implemented for `&T` and for `Option<T>`, the latter rendering
  nothing when `None` — the rule `Attr` already follows, and what lets
  `{@render slots.get(…)}` stand without a guard around it.

### Changed

- **Breaking.** Slots are filled the way web components fill them: a direct child
  of a component element carrying `slot="x"` goes into the `x` slot, the element
  included, and several children may name the same slot — they land there in the
  order written. `<slot>` is now *only* a placeholder; a named `<slot>` inside a
  component element no longer fills anything. Rewrite
  `<Frame><slot name="footer">© 2026</slot></Frame>` as
  `<Frame><span slot="footer">© 2026</span></Frame>`, and forwarding —
  `<slot name="footer"><slot name="footer"/></slot>` — as
  `<slot name="footer" slot="footer"/>`. A bare `<slot/>` inside a component
  element still forwards the default slot, unchanged. The `slot` attribute is
  consumed rather than rendered, and outside a component element it stays an
  ordinary attribute, so a template can address a browser-side custom element's
  shadow slots.

## [0.1.2] - 2026-07-21

### Changed

- Templates are described on their own terms rather than by comparison to
  another template language, in the README, the crate docs and the agent skill.
  `damask-template`'s keywords follow.
- The licence file is `LICENSE`, the `-MIT` suffix having distinguished it only
  from an Apache copy that no longer exists.
- The Tree-sitter grammar moved to its own repository,
  [tree-sitter-damask](https://github.com/JWo1F/tree-sitter-damask), which is
  what Zed can clone; the Zed extension pins it by revision.

## [0.1.1] - 2026-07-20

### Changed

- Each published crate carries its own README. `damask-macros`, `damask-template`
  and `damask-lsp` previously showed the whole project README on crates.io, which
  told a reader to depend on something other than the crate they were looking at.
- `damask-macros` is described by the derive it actually exports. Its published
  description named a `component!` macro that does not exist.

### Fixed

- Broken intra-doc links: `render_into` is documented on `Render`, which declares
  it, rather than on `Component`.

## [0.1.0] - 2026-07-19

### Added

- Props a call site may skip. A prop whose type is `Option<_>` may be left out
  of a `<Comp …/>` tag and arrives as `None`; `#[component(default)]` on the
  struct extends that to every prop, filling the skipped ones from the struct's
  `Default`. A required prop left out is still a compile error, and now names
  the prop. A quoted value also reaches an `Option` prop directly
  (`detail="…"` → `Some("…")`), where it previously needed `{Some(…)}`. Spec §7.
- Templates are laid out rather than copied verbatim: a whitespace run
  containing a newline becomes one newline plus the node's nesting depth, so a
  `{# … #}` comment or a `{#if}` tag no longer leaves a blank line in the
  output. `renderers::Whitespace` — and the crate's `pretty` / `minify`
  features — then choose whether the renderer adds the call site's depth to a
  component's markup, or crushes each run to the single space it renders as
  (~14% of a page). Spec §8.5 gives the argument for why none of it can change
  the rendered document.
- `Renderer::write_text`, for the literal text between a template's tags — the
  only markup a renderer may lay out. Defaults to `write_raw`, so a renderer
  that does not format needs no change.
- `Renderer::push_indent` / `pop_indent` / `set_verbatim` / `close_line`, all
  defaulting to no-ops, so the trait stays object-safe and existing renderers
  keep working.

### Changed

- `Slots::render` takes the declaring `<slot>`'s depth, which it applies to a
  fill (written in the caller, laid out from the caller's root) but not to the
  fallback (the declaring template's own markup).

- **Components via `#[derive(Component)]`** on a normal struct (its fields are
  the props). Methods are a plain `impl`; `#[template(path = "…")]` overrides
  the paired template.
- **Brace-tag HTML template language** (`damask-template`), parsed into an
  HTML-aware tree:
  - `{ … }` is a Rust block — prints its value (escaped) or runs as a statement.
  - `{@html}` (raw), `{@render}` (render a snippet/fragment),
    `{#if}`/`{:else if}`/`{:else}`/`{/if}`, `{#each E as p[, i]}`/`{/each}`,
    `{#snippet name(params)}`/`{/snippet}`.
  - `{use path}` — a Rust `use`, scoped to the enclosing HTML element.
  - **HTML and component elements**: lowercase `<div>` renders; capitalized
    `<Component attr={e}>` is built from its attributes and rendered (omitting
    a field that is not skippable is a compile error).
  - **Slots**: `<slot/>` / `<slot name="x">fallback</slot>` render caller-passed
    content, or the slot's own body when unfilled. Slots are not struct fields —
    they travel as a `Slots` argument to `Render::render_slots`, so a template
    can declare any number of them without the struct changing. Callers fill
    them with `<Comp>…</Comp>` and `<slot name="x">…</slot>`, or from Rust with
    `Component::render_with`. A bare `<slot/>` inside a component element stays
    a placeholder, so it forwards the enclosing component's default slot. The
    trade is that names are matched at render time, not compile time.
  - The parser balances nested braces and respects string/char literals, so
    struct literals work inside `{@render …}` and attribute values.
- **Attribute values that can decline to render**, via the `Attr` trait.
  `name={expr}` asks the value's type how to appear: `bool` renders a bare
  `disabled` when true and *nothing* when false (an attribute's presence is what
  HTML reads, so `disabled="false"` is a disabled control), and `Option<T>`
  renders nothing for `None`. Implemented for the string types, the numbers,
  `bool` and `Option` of those. There is deliberately no blanket impl over
  `Display`, which would collide with exactly the two impls that matter — a type
  of your own implements `Attr` or reaches the template as a string.
- **Interpolation in quoted attribute values**: `class="px-3 {self.tone()}"`
  holds literal and `{ … }` parts, each escaped. A value with no holes still
  lowers to literal text, so the common case costs nothing.
- **Class lists.** `class` (and only `class`) additionally accepts:
  - `class=[Some("a"), None, "b", { "c": cond }]` — a list whose entries may be
    strings, `Option`s of them, or a map of conditional names. A literal `None`
    is dropped at compile time, since a bare `None` has no type to infer.
  - `class={ "c": cond, "d": cond }` — the map form alone. Told apart from an
    ordinary `class={expr}` by a top-level `:` that is not part of a `::` path.
  - `class:name={cond}` — a directive that adds or removes one name and
    **takes precedence** over whatever the list produced.

  Everything lands in one `ClassList`, which dedupes and keeps first-mention
  order; an empty result omits the attribute.
- **Attribute spreading**: `<button {...expr}>` splices a prepared run of
  attributes, for the ones a component cannot name — a computed name such as
  `data-<controller>-target`, or a map. The `AttrSpread` trait is implemented
  for `&'static str` (markup the author wrote — the lifetime is what keeps a
  request-derived value out) and for `[(K, V)]`/`Vec<(K, V)>`, which escapes and
  is where anything derived from state belongs.
- **Sibling template resolution** via `Span::local_file()` (stable on Rust
  1.88+): `<name>.dmk` is found next to the struct with no build script; editing
  it triggers a rebuild through an emitted `include_bytes!`.
- **`Renderer` trait** — the extensibility seam owning the output buffer and
  escaping policy — with the built-in `HtmlRenderer` (escapes `& < > " '`) and
  the `StringRenderer` core.
- **`Render` trait + composition**: components and `fragment(|r| …)` closures are
  both renderable; `{@render …}` embeds either, and slot content is a borrowed
  `&dyn Render` (or a template `{#snippet}`), so it stays on the caller's stack.
- **`damask-lsp`** language server: parse diagnostics and in-tag completion of a
  component's fields and methods.
- **Zed extension** with a `tree-sitter-damask` grammar (Rust injected into `{ }`
  tags, HTML into text) wired to `damask-lsp`.
- **Agent skill** (`skills/damask`) for authoring components.

Damask is HTML-only: there is no per-language host extension, and `{ … }` always
HTML-escapes.

[Unreleased]: https://github.com/jwo1f/damask/compare/v0.10.1...HEAD
[0.10.1]: https://github.com/jwo1f/damask/compare/v0.10.0...v0.10.1
[0.10.0]: https://github.com/jwo1f/damask/compare/v0.9.0...v0.10.0
[0.8.1]: https://github.com/jwo1f/damask/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/jwo1f/damask/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/jwo1f/damask/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/jwo1f/damask/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/jwo1f/damask/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/jwo1f/damask/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/jwo1f/damask/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/jwo1f/damask/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/jwo1f/damask/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/jwo1f/damask/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/jwo1f/damask/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/jwo1f/damask/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/jwo1f/damask/releases/tag/v0.1.0
