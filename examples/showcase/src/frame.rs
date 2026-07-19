use damask::Component;

// Two slots: `<slot/>` renders the default content, `<slot name="footer"/>` the
// named one. Neither appears on the struct — a template can add or drop a slot
// without touching this type.
#[derive(Component)]
pub struct Frame {
    pub title: String,
}
