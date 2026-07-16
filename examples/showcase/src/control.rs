use rsc::Component;

/// Attribute forms that render conditionally.
///
/// `disabled` is a `bool`, so it renders a bare `disabled` or nothing at all —
/// never `disabled="false"`, which HTML reads as disabled. `placeholder` is an
/// `Option`, so `None` omits it rather than rendering an empty one. The `class`
/// list mixes an `Option`, a plain name and a conditional map, and the
/// `class:` directive gets the last word over all of them.
#[derive(Component)]
pub struct Control {
    pub disabled: bool,
    pub placeholder: Option<String>,
    pub extra: Option<&'static str>,
    pub invalid: bool,
    pub compact: bool,
    /// Markup the author wrote — `&'static str`, so a value from a request
    /// cannot reach it.
    pub wiring: &'static str,
    /// Name/value pairs, escaped on the way out. Where anything derived from
    /// state belongs.
    pub data: Vec<(String, String)>,
}
