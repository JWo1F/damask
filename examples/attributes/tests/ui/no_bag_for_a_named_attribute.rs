//! `<Strict class="field"/>` — `class` is ident-shaped, so it could have been a
//! prop and lowering leaves the choice to method resolution: the setter call,
//! and beside it the fallback trait of the same name. `Strict` declares neither
//! a `class` prop nor a `#[prop(rest)]` field, so there is no setter to win and
//! no bag to fall through to, and the impl's bound is where the message lives.
//!
//! The bound is on the impl rather than on the methods so that the fallback is
//! not a candidate for anything but a props builder — otherwise a template's own
//! `{row.class()}` resolved to it. This is what that costs the diagnostic.

#[allow(non_camel_case_types)]
trait __DamaskRest_class {
  fn __damask_literal_class(self, text: &'static str) -> Self;
}

impl<T: damask::props::Rest> __DamaskRest_class for T {
  fn __damask_literal_class(self, text: &'static str) -> Self {
    damask::props::Rest::__damask_rest_static(self, "class", text)
  }
}

fn main() {
  let _ = damask_attributes::strict::Strict::__damask_props()
    .__damask_literal_class("field")
    .__damask_build();
}
