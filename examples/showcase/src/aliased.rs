//! A component whose generated code never says `::damask`.
//!
//! `#[component(crate = …)]` exists for a framework that re-exports Damask
//! rather than having its users depend on it. This stands in for such a
//! framework: `framework::view` is the only path the derive is given, and what
//! it generates — the trait impls, the prop builder, the attribute and
//! child-component calls inside the template — has to reach the crate through
//! it.

/// Damask under someone else's name, as a re-exporting framework would have it.
///
/// Private on purpose: a *public* re-export of a whole crate gives rustc a
/// second visible path to every item in it, and its diagnostics fall back to
/// bare type names when a path is not unique.
mod framework {
    pub use damask as view;
}

#[derive(framework::view::Component)]
#[component(crate = crate::aliased::framework::view)]
pub struct Aliased {
    pub who: String,
    pub tone: String,
    pub emphatic: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use damask::Component;

    #[test]
    fn renders_through_the_configured_path() {
        let out = Aliased {
            who: "Ada".into(),
            tone: "warm".into(),
            emphatic: true,
        }
        .render();

        assert!(out.contains(r#"class="warm""#), "{out}");
        assert!(out.contains("Hello Ada!"), "{out}");
        assert!(out.contains("<strong>!</strong>"), "{out}");
    }
}
