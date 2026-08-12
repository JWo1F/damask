use damask::Component;
use std::collections::BTreeMap;

/// The `data` forms, which expand one value into a run of `data-*` attributes.
///
/// `attrs` is a map, as a Rails view would pass one; `extra` is an ordered pair
/// list, and an `Option` of one, so a whole group can be absent. The inline map
/// adds two more entries whose values are not strings: a number carries its
/// display, and a `bool` renders a bare attribute or nothing, the same rule
/// `disabled` follows on [`Control`](crate::control::Control).
///
/// The longhand `data-controller` beside them is an ordinary attribute and
/// stays one — the set does not collect it, so its value is held to `Attr`.
#[derive(Component)]
pub struct Wired {
    pub attrs: BTreeMap<String, String>,
    pub extra: Option<Vec<(&'static str, String)>>,
    pub index: u32,
    pub open: bool,
}
