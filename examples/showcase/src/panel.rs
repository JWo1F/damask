use crate::button::Button;
use damask::Component;

// Composition via the string path: `{@html self.button.render()}` writes the
// child's finished (already-escaped) HTML raw. Same output as `Card`'s
// `{@render …}`, at the cost of one intermediate String.
//
// `render()` resolves without importing the `Component` trait: the derive brings
// it into scope inside the generated body.
#[derive(Component)]
pub struct Panel {
    pub button: Button,
}
