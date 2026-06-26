use crate::button::Button;
use rsc::Component;

// Composition via `<%+ self.button %>`: the child renders directly into the
// parent's buffer (no intermediate String).
#[derive(Component)]
pub struct Card {
    pub button: Button,
}
