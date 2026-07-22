use damask::Component;

/// The woven ground: an ogee net, tiled behind the whole document.
///
/// Damask is a figured weave, so the site is printed on one. The net is not
/// authored here — `site.js` draws it from the minute of the day, so the cloth a
/// reader sees at nine in the morning is not the one they see at nine at night,
/// and it is redrawn on every minute they leave the page open.
///
/// What *is* authored here is the host and its fallback tile. With scripting off
/// the net below never changes, which is the whole of what is lost.
#[derive(Component)]
pub struct Ground {}
