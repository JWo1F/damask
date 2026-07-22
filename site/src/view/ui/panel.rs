use damask::Component;

/// A code panel with a filename tab.
///
/// `code` arrives already highlighted, as the bare `<pre>` from
/// [`Highlighter::pre`](crate::highlight::Highlighter::pre) — the caption below
/// is this component's own, so the framed form would caption the same code
/// twice. It is emitted with `{@html}`; everything that reaches this field went
/// through the highlighter, which escapes the code it was given, and nothing
/// user-submitted has a path here.
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
