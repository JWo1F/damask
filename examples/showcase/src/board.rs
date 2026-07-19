use damask::Component;

/// A call site skipping props: `Notice`'s `Option`s and, since `Theme` is
/// `#[component(default)]`, any of `Theme`'s at all.
#[derive(Component)]
pub struct Board {
    pub log: String,
}
