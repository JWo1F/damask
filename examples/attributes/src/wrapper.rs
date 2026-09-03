use damask::{Attrs, Component};

use crate::passthrough::Passthrough;

/// The same bag, forwarded rather than spread.
///
/// A wrapper takes whatever its own call site wrote and hands it to the control
/// inside it with `{...self.attrs}`, which is how a component that is really a
/// thin layer over another one stays transparent.
#[derive(Component, Default)]
#[component(default)]
pub struct Wrapper {
    #[prop(rest)]
    pub attrs: Attrs,
}
