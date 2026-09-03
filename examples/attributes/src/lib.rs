//! An attribute a component does not name, carried through to the page.
//!
//! `<Passthrough data-signup-target="email" aria-label="Email" autofocus/>`
//! names three attributes the component has never heard of, and all three
//! reach the `<input>` it renders. What makes that work is one field:
//!
//! ```ignore
//! #[prop(rest)]
//! pub attrs: Attrs,
//! ```
//!
//! and one spread in its template, `{...self.attrs}`, which decides where they
//! land. Without it each would have to be a declared prop, or the call site
//! would have to hand-assemble them into markup.
//!
//! # The two rules
//!
//! **A declared prop wins over the bag.** `kind` is a prop of
//! [`Passthrough`](passthrough::Passthrough), so `kind="email"` is read by the
//! component and never reaches `attrs`. Which of the two an attribute takes is
//! settled when it compiles, by method resolution, and nothing in the markup
//! says which — that is the point.
//!
//! **An element's own attributes win over the spread.** `Passthrough` writes
//! its own `type`, so a `type="tel"` at the call site is dropped rather than
//! written a second time into the same tag. A component that wants a call site
//! to override something declares it as a prop, which is what `kind` is for.
//!
//! # The bag is opt-in
//!
//! [`Strict`](strict::Strict) declares none, so an attribute it does not name
//! is a build failure — which is what keeps a typo from being rendered into the
//! page instead of reported. `tests/ui` is what that failure looks like.

pub mod page;
pub mod passthrough;
pub mod strict;
pub mod wrapper;
