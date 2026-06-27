//! Compile-fail coverage for the `Component` derive's attribute diagnostics.
//!
//! These cases fail during attribute parsing — before template resolution — so
//! they need no `.rsc` files and produce machine-independent error messages,
//! which keeps the `.stderr` snapshots portable. Template/parse error paths are
//! covered by unit tests in `rsc-macros` and `rsc-template`, because file
//! resolution and absolute paths do not survive trybuild's sandbox cleanly.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
