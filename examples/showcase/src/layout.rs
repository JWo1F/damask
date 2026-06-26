use rsc::{Component, Render};

// A slot/children host: `children` is any `Render` — a fragment closure or
// another component — dropped in at `<%+ self.children %>`. Generic (not
// `Box<dyn Render>`) so the child can borrow the caller's data with no boxing.
#[derive(Component)]
pub struct Layout<C: Render> {
    pub children: C,
}
