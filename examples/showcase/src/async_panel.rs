use crate::button::Button;
use damask::Component;

// Awaits directly (`load_label`), embeds a plain sync `Button` (proving a sync
// child renders through the free `AsyncRender` it gets from `Render`), renders
// a snippet that itself awaits, and falls back a `<slot>` that awaits too.
#[derive(Component)]
pub struct AsyncPanel {
    pub label: String,
}

impl AsyncPanel {
    pub async fn load_label(&self) -> String {
        self.label.clone()
    }
}
