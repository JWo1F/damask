use rsc::Component;

// `theme.css.rsc` -> the CssRenderer, which is pass-through: `<%= … %>` is not
// HTML-escaped here, because that would corrupt CSS.
#[derive(Component)]
pub struct Theme {
    pub accent: String,
}
