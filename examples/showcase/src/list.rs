use rsc::Component;

// Control flow: the `for` loop's braces open in one `<% %>` tag and close in
// another. The generated body is parsed as a single Rust block, so this works.
#[derive(Component)]
pub struct List {
    pub items: Vec<String>,
}
