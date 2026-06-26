use crate::button::Button;
use rsc::Component;

// Composition via the string path: `self.button.render()` returns the child's
// finished (already-escaped) HTML, written raw with `<%- … %>`. Produces the
// same output as `Card`'s `<%+ … %>`, at the cost of one intermediate String.
//
// `render()` resolves without importing the `Component` trait method: the derive
// brings it into scope inside the generated body.
#[derive(Component)]
pub struct Panel {
    pub button: Button,
}
