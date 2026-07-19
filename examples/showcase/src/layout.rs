use damask::Component;

// A slot host: `<slot/>` in layout.dmk renders whatever the caller passed for
// the default slot. Slots are not fields, so the struct stays plain — callers
// supply content with `render_with`, or as `<Layout>…</Layout>` in a template.
#[derive(Component)]
pub struct Layout;
