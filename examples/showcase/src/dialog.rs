use damask::Component;

// Asking about slots: `slots` is the caller's fills, in scope for any `{ … }`
// tag. `has_default()` / `has(name)` guard the *wrapper* — a `<slot>`'s fallback
// stands in for missing content, so it cannot answer whether the `<p>` and
// `<footer>` should exist at all. The two ways to place a fill are both here:
// `<slot/>` resolves implicitly, `{@render slots.get(…)}` by name.
#[derive(Component)]
pub struct Dialog {
    pub title: String,
}
