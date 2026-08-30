# Release process

Before pushing any change: add an entry to `CHANGELOG.md` under `[Unreleased]`
(matching its existing style) for every notable change, and bump the
workspace version in `Cargo.toml` (`[workspace.package].version`, which the
member crates inherit via `version.workspace = true`) when the change warrants
a release — then move the `[Unreleased]` entries under a new dated version
heading, following the pattern of the existing release headings.

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
