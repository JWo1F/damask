//! `<Passthrough attrs={r#"data-controller="signup""#}/>` — the older spelling,
//! where the attributes were assembled into markup by hand. The bag holds pairs
//! it escapes one at a time, so a string is refused and the note says what to
//! write instead.

use damask_attributes::passthrough::Passthrough;

fn main() {
  let _ = Passthrough::__damask_props()
    .attrs(r#"data-controller="signup""#)
    .__damask_build();
}
