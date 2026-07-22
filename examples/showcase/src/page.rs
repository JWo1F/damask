use damask::Component;

// Component elements + scoped import: `{use}` brings `Frame` into scope for the
// enclosing `<div>`, then `<Frame …>…</Frame>` builds and renders it. The `<p>`
// carries no `slot`, so it fills the default slot; the two `slot="footer"`
// children both land in the footer, in the order written.
#[derive(Component)]
pub struct Page {
    pub heading: String,
    pub body: String,
    pub year: u32,
}
