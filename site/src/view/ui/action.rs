use damask::Component;

/// A call to action.
///
/// A link, never a button: everything this site asks a reader to do is navigate
/// somewhere. The label is the default slot so a call site can put an icon
/// beside it without a second prop.
#[derive(Component)]
pub struct Action {
    pub href: String,
    pub primary: bool,
    pub class: String,
}

impl Action {
    fn skin(&self) -> &'static str {
        if self.primary {
            "bg-ink text-paper border-ink hover:bg-madder hover:border-madder"
        } else {
            "bg-transparent text-ink border-rule hover:border-ink-soft hover:bg-ink/[0.035]"
        }
    }
}
