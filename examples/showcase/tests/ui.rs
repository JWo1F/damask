//! Compile-fail coverage for props a call site failed to provide.
//!
//! It lives here rather than in `crates/rsc` because it needs a *built*
//! component to construct: trybuild compiles each case in its own scratch
//! directory, where a `#[derive(Component)]` would find no sibling `.rsc` file.
//! The cases therefore drive `Notice`'s generated builder directly — which is
//! exactly what lowering a `<Notice …/>` tag emits, as
//! `lower::tests::component_element_construction` pins.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
