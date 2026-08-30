use damask::Component;

// The `.dmk` awaits `load_name`, so the derive emits `AsyncRender`/
// `AsyncComponent` instead of `Render`/`Component` — rendered with
// `.render_async().await`, not `.render()`.
#[derive(Component)]
pub struct AsyncGreeting {
    pub name: String,
}

impl AsyncGreeting {
    pub async fn load_name(&self) -> String {
        self.name.clone()
    }
}
