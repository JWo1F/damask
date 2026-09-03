//! Compile-fail coverage for the two ways an attribute is refused.
//!
//! Like the showcase's, these drive the generated builder directly rather than
//! writing a tag: trybuild compiles each case in its own scratch directory,
//! where a `#[derive(Component)]` would find no sibling `.dmk`. The calls here
//! are exactly what lowering a tag emits — `lower::tests` in `damask-template`
//! pins that.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
