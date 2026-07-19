//! Typestate and conversions for the prop builder the `Component` derive
//! generates.
//!
//! A call site in a template names only the props the author wrote, and it is
//! compiled in a different crate from the struct it is building — so it cannot
//! know which fields it left out, nor what those fields default to. The derive
//! knows both, and answers by generating a builder: one setter per field, and a
//! `__damask_build` that is reachable only once every *required* prop has been set.
//!
//! Nothing here is meant to be named by hand.
//!
//! # Which props may be skipped
//!
//! The builder carries one type parameter per required prop. Setting the prop
//! swaps that parameter from a marker the derive named after it to [`Set`], and
//! `__damask_build` requires every one of them to be [`Provided`]. A forgotten prop
//! is therefore a trait-bound error naming the prop, at the call site.
//!
//! A prop whose type is `Option<_>` needs no marker: leaving it out yields
//! `None`. Neither does any prop of a struct marked `#[component(default)]`,
//! whose builder starts from `Default::default()`.
//!
//! # Reaching an `Option` prop with a quoted value
//!
//! A setter takes its prop's type exactly, so that passing a value to it behaves
//! as assigning to the field did — `&Vec<T>` still coerces to a `&[T]` prop,
//! `count={2 + 8}` still infers to whatever integer the prop is. That leaves the
//! conversion a quoted value needs to happen on the *value* side, where the
//! prop's type is what the result is inferred from:
//!
//! ```text
//! detail: Option<String>     detail="check the log"   → Some("check the log")
//!                            detail="row {self.n}"    → Some("row 3")
//!                            detail={self.detail}     → passed through
//!                            (omitted)                → None
//! ```
//!
//! An interpolated value is already a `String`, and `String` reaches both a
//! `String` prop and an `Option<String>` one through `Into`. Static text is the
//! case `Into` cannot serve — no `From<&'static str> for Option<String>` exists,
//! and adding one is not ours to do — so [`literal`] stands in for it.

/// A required prop that has been supplied.
pub struct Set;

/// Satisfied only by [`Set`] — the bound `__damask_build` places on each required
/// prop's marker.
///
/// The unsatisfied case is the diagnostic, so it is phrased here: the failing
/// type is the marker the derive named after the missing prop, and `{Self}`
/// puts that name in the message.
#[diagnostic::on_unimplemented(
    message = "missing a required prop: {Self}",
    label = "a required prop was not given",
    note = "a prop is required unless its type is `Option<_>`, which makes leaving it out mean `None`",
    note = "`#[component(default)]` on the struct makes every prop skippable, filling the rest from its `Default`"
)]
pub trait Provided {}

impl Provided for Set {}

/// A string type buildable from *either* form a quoted attribute value arrives
/// in: static text, or an interpolated `String`.
///
/// Requiring both is not incidental. It is what makes [`literal`] resolve to
/// exactly one conversion, because it excludes — by construction rather than by
/// a list — the one type that would otherwise fit both of [`FromLiteral`]'s
/// impls: `&'static str`, which no `String` converts into.
///
/// Blanket-implemented, so `String`, `Cow<'static, str>`, `Box<str>`, `Rc<str>`,
/// `Arc<str>` and any type of your own with both conversions qualify.
pub trait FromText: From<&'static str> + From<String> {}

impl<T: From<&'static str> + From<String>> FromText for T {}

/// The value is the prop's own type.
pub struct Direct;
/// The value is what the prop's `Option` wraps.
pub struct Wrapped;

/// How static attribute text becomes a prop, given the prop's type.
///
/// `M` is what keeps the two impls from overlapping — a prop is reached either
/// as itself or through its `Option` — and is always inferred.
#[diagnostic::on_unimplemented(
    message = "a quoted attribute value cannot become `{Self}`",
    label = "no conversion from text to this prop's type",
    note = "a quoted value needs `From<&'static str>` for the prop's type (or for what its `Option` wraps); pass a `{{ … }}` value instead"
)]
pub trait FromLiteral<M>: Sized {
    fn from_literal(text: &'static str) -> Self;
}

impl<T: From<&'static str>> FromLiteral<Direct> for T {
    fn from_literal(text: &'static str) -> Self {
        T::from(text)
    }
}

// Reached only where the prop's `Option` wraps a type static text converts into
// *and* an interpolated one would — the pair that rules out `&'static str`,
// whose `Option` the impl above already serves through `From<T> for Option<T>`.
impl<T: FromText> FromLiteral<Wrapped> for Option<T> {
    fn from_literal(text: &'static str) -> Self {
        Some(T::from(text))
    }
}

/// Convert the static text of a quoted attribute into the prop it is being
/// passed to. Emitted by the template lowering.
pub fn literal<M, T: FromLiteral<M>>(text: &'static str) -> T {
    T::from_literal(text)
}
