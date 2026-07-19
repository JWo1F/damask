use damask::Component;

/// Props a call site may leave out.
///
/// `title` is required: forgetting it is a compile error at the call site. The
/// others are `Option`s, which is the type saying what absence means — a skipped
/// one arrives as `None`, and renders as no attribute at all. A skippable flag
/// is `Option<bool>` rather than `bool` so that the type, not a convention, is
/// where a caller reads whether it has to pass anything.
///
/// An `Option` prop is not only skippable but takes a quoted value directly:
/// `detail="…"` arrives as `Some`, with no `Some(…)` at the call site.
#[derive(Component)]
pub struct Notice {
    pub title: String,
    pub detail: Option<String>,
    pub tone: Option<&'static str>,
    pub dismissible: Option<bool>,
}
