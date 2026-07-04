use rsc::{Component, Render};

// Slots: `<slot/>` renders the default children, `<slot name="footer"/>` renders
// the named footer slot. Slot fields are generic `Render` fields.
#[derive(Component)]
pub struct Frame<Body: Render, Footer: Render> {
    pub title: String,
    pub children: Body,
    pub footer: Footer,
}
