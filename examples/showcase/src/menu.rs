use damask::Component;

// Snippets + render-props: `{#snippet item(label)}` defines a parameterized
// fragment, `{@render item(label)}` invokes it per element.
#[derive(Component)]
pub struct Menu {
    pub labels: Vec<String>,
}
