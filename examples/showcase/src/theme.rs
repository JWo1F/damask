use rsc::Component;

/// `#[component(default)]` — every prop may be skipped, and the ones a call site
/// omits come from the struct's own `Default`.
///
/// Worth the opt-in only when the defaults are meaningful, as here: `Default` is
/// hand-written so a skipped prop lands on a real value rather than on the zero
/// one a derived impl would give it.
#[derive(Component)]
#[component(default)]
pub struct Theme {
    pub accent: String,
    pub label: String,
    pub dense: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            accent: "indigo".into(),
            label: "Theme".into(),
            dense: false,
        }
    }
}
