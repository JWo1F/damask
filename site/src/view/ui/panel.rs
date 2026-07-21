use damask::Component;

/// A code panel with a filename tab.
///
/// `code` arrives already highlighted — the markdown pipeline and the home
/// page's front matter both hand over finished `<pre>` markup — so it is emitted
/// with `{@html}`. Everything that reaches this field went through
/// [`Highlighter::block`](crate::highlight::Highlighter::block), which escapes
/// the code it was given; nothing user-submitted has a path here.
#[derive(Component)]
pub struct Panel {
    pub name: String,
    pub code: String,
    /// Marks the panel that is *output* rather than source — the one thing the
    /// hero has to distinguish, since the point is that two files became a third.
    pub derived: bool,
    pub class: String,
}

impl Panel {
    fn dot(&self) -> &'static str {
        if self.derived {
            "bg-madder"
        } else {
            "bg-ink-faint"
        }
    }

    fn name_skin(&self) -> &'static str {
        if self.derived {
            "text-madder"
        } else {
            "text-ink-soft"
        }
    }
}
