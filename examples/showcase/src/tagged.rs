use damask::Component;
use std::fmt::Display;

/// The builder carries the component's own generics, so a generic component
/// takes skippable props like any other.
#[derive(Component)]
pub struct Tagged<'a, T: Display> {
    pub value: T,
    pub note: Option<&'a str>,
}
