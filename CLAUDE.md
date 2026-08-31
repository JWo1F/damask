# Release process

Before pushing any change: add an entry to `CHANGELOG.md` under `[Unreleased]`
(matching its existing style) for every notable change, and bump the
workspace version in `Cargo.toml` (`[workspace.package].version`, which the
member crates inherit via `version.workspace = true`) when the change warrants
a release — then move the `[Unreleased]` entries under a new dated version
heading, following the pattern of the existing release headings.

**A version bump is not finished until it is tagged.** Every release of the
workspace version gets an annotated tag on the release commit, named `vX.Y.Z`
and messaged `Damask X.Y.Z`, pushed with it:

```sh
git tag -a v0.6.0 -m "Damask 0.6.0"
git push origin v0.6.0
```

The reason is that publishing to crates.io stopped after 0.3.2 — `damask`,
`damask-template` and `damask-macros` are all on the registry up to that version
and no further — so anything wanting 0.4.0 or later reaches these crates over
git, and a git dependency has three ways to say which commit it wants. A `branch` moves under the dependent without warning.
A `rev` is a short hash that says nothing about what it contains, so nobody can
tell from a manifest whether they are on the current release or eleven commits
behind it, and updating means reading this repository's log. A `tag` is the
version number, so `tag = "v0.6.0"` in somebody else's `Cargo.toml` is legible
and is exactly as immutable as a rev. That is what `../ironstone` uses, and it
can only do so if the tag exists.

Tagging lapsed once already — the workspace reached 0.6.0 while the newest tag
was `v0.3.2`, which is why `ironstone` was pinned to a bare `rev`. If you find
an untagged release, tag its commit rather than skipping the number.

# The Zed extension's version

Any push to `master` that changes something the extension ships must bump
`version` in `editors/zed/extension.toml` — the queries and `config.toml` under
`editors/zed/languages/`, the pinned grammar `rev`, or the extension crate
itself. Zed decides whether an installed extension is stale by comparing that
number, so a change shipped without a bump reaches nobody: the fix is on
`master` and every editor keeps running the old copy.

Bump the minor for new or changed behaviour, the patch for a fix that leaves
behaviour alone. Keep `version` in `editors/zed/Cargo.toml` on the same number —
Zed reads only the manifest, but the two started in step and a crate left behind
just raises the question of which one is the extension's version.

This is separate from the workspace version above, which tracks the published
crates and moves only when a release warrants it. A push may bump either, both,
or neither.
