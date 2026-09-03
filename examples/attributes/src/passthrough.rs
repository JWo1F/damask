use damask::{Attrs, Component};

/// A control that carries the attributes it does not name.
///
/// `kind` and `value` are props: the component knows what they mean and reads
/// them. Everything else a call site writes — `data-controller`, `aria-label`,
/// `type`, `autofocus` — lands in `attrs` because of `#[prop(rest)]`, and
/// reaches the page through the `{...self.attrs}` in its template.
///
/// Without the bag each of those would have to be a declared prop, or the call
/// site would have to hand-assemble markup into one.
#[derive(Component, Default)]
#[component(default)]
pub struct Passthrough {
    /// A declared prop wins over the bag, whatever the call site writes — so
    /// `kind="email"` is read here and never reaches `attrs`.
    pub kind: Option<String>,
    pub value: Option<String>,
    #[prop(rest)]
    pub attrs: Attrs,
}

impl Passthrough {
    pub fn kind(&self) -> &str {
        self.kind.as_deref().unwrap_or("text")
    }
}
