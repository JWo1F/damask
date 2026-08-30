# Release process

Before pushing any change: add an entry to `CHANGELOG.md` under `[Unreleased]`
(matching its existing style) for every notable change, and bump the
workspace version in `Cargo.toml` (`[workspace.package].version`, which the
member crates inherit via `version.workspace = true`) when the change warrants
a release — then move the `[Unreleased]` entries under a new dated version
heading, following the pattern of the existing release headings.
