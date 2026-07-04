use rsc::Component;

// Component elements + scoped import: `{#use}` brings `Frame` into scope for the
// enclosing `<div>`, then `<Frame …>…</Frame>` builds and renders it, filling
// its default slot with <p> and its "footer" slot.
#[derive(Component)]
pub struct Page {
    pub heading: String,
    pub body: String,
    pub year: u32,
}
