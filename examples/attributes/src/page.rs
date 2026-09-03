use damask::Component;

use crate::passthrough::{Passthrough, Seeded};
use crate::strict::Strict;
use crate::wrapper::Wrapper;

/// The call sites, which is where the whole feature is visible.
///
/// Read `page.dmk` next to this: every attribute on a `<Passthrough/>` there is
/// either a prop the component declares or one it has never heard of, and
/// nothing in the markup says which. That is the point — the call site writes
/// HTML attributes, and where each one goes is settled when it compiles.
#[derive(Component)]
pub struct Page {
    pub email: Option<String>,
    /// Attributes assembled in Rust rather than written down, spread with
    /// `{...}`. Values reach the page escaped, so this is where anything derived
    /// from state belongs.
    pub tracking: Vec<(String, String)>,
}

impl Page {
    pub fn row(&self) -> Row {
        Row {
            label: "Email address",
            required: true,
        }
    }
}

/// A value the template calls a method on, named after an attribute a component
/// tag in the same template writes.
///
/// `class="field"` on the `<Passthrough/>` above is what puts a fallback trait
/// called `class` in this template's scope, and `{row.class()}` on a plain
/// `<label>` has to keep reaching *this* method. It did not while the fallback's
/// bound was on its methods rather than on its impl: a setter takes `self` by
/// value, a by-value candidate is picked before an autoref one, so a blanket
/// impl beat an inherent `&self` method — and said so as errors about a
/// component nobody had written. A reference was unaffected, which is what made
/// it worth a test: the same call compiled or not depending on whether the
/// template held the value or borrowed it.
pub struct Row {
    pub label: &'static str,
    pub required: bool,
}

impl Row {
    pub fn class(&self) -> &'static str {
        if self.required { "row required" } else { "row" }
    }
}
