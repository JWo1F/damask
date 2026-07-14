use rsc::Component;

// Slot forwarding: shell.rsc passes its own slots straight through to `Frame`,
// so a caller's content lands two components deep. A bare `<slot/>` inside
// `<Frame>` forwards the default slot; the named one nests a placeholder inside
// a fill, which is how a *named* slot forwards.
#[derive(Component)]
pub struct Shell {
    pub title: String,
}
