//! `<Strict data-anything="1"/>` — `Strict` declares no `#[prop(rest)]` field,
//! so there is nowhere for an attribute it does not name to go. A hyphenated
//! name could never be a method, so this is what lowering emits for it: the
//! blanket trait, whose bound is where the message lives.

trait __DamaskRestAny {
  fn __damask_rest_static_any(self, name: &'static str, value: &'static str) -> Self
  where
    Self: damask::props::Rest;
}

impl<T> __DamaskRestAny for T {
  fn __damask_rest_static_any(self, name: &'static str, value: &'static str) -> Self
  where
    Self: damask::props::Rest,
  {
    damask::props::Rest::__damask_rest_static(self, name, value)
  }
}

fn main() {
  let _ = damask_attributes::strict::Strict::__damask_props()
    .__damask_rest_static_any("data-anything", "1")
    .__damask_build();
}
