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

/// A component whose `Default` seeds the bag.
///
/// `#[component(default)]` is what makes the builder start from `Default` rather
/// than build the struct field by field, and it is the only reason a component
/// needs it — none of the components above do, since every one of their props is
/// either an `Option` or the bag itself, and both may be left out regardless.
///
/// The bag is merged rather than replaced, because it is a collection: a call
/// site *adds* attributes, it does not write the field. Otherwise `class="btn"`
/// here would vanish the moment a call site wrote any attribute at all, with
/// nothing saying so. A name in both takes the call site's value and keeps the
/// default's position.
#[derive(Component)]
#[component(default)]
pub struct Seeded {
    #[prop(rest)]
    pub attrs: Attrs,
}

impl Default for Seeded {
    fn default() -> Self {
        let mut attrs = Attrs::new();
        attrs.insert_static("class", "btn");
        attrs.insert_static("data-role", "button");
        Self { attrs }
    }
}
