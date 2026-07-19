//! A third-party [`Renderer`] proving the extensibility seam: the same
//! macro-generated components can be driven by a renderer Damask never knew about.

use damask::Renderer;
use std::fmt::Display;

/// A renderer that upper-cases everything written to it.
#[derive(Default)]
pub struct UpcaseRenderer {
    buf: String,
}

impl UpcaseRenderer {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Renderer for UpcaseRenderer {
    fn write_raw(&mut self, s: &str) {
        self.buf.push_str(&s.to_uppercase());
    }

    fn write_escaped(&mut self, value: &dyn Display) {
        // This renderer's "escaping policy" is simply to upper-case.
        self.buf.push_str(&value.to_string().to_uppercase());
    }

    fn finish(self: Box<Self>) -> String {
        self.buf
    }
}
