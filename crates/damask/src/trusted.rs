//! Markup that is already safe to write, and the seam that recognises it.
//!
//! Everything a template interpolates with `{ … }` is escaped, because the
//! overwhelmingly common thing to interpolate is *data* — a name, a subject
//! line, a number somebody typed. [`Trusted`] is the exception a document needs
//! anyway: a value that is markup rather than data, built by [`tag!`](crate::tag)
//! or by a component that has already rendered, and therefore written through
//! untouched.
//!
//! Rails draws the same distinction with `SafeBuffer` and `html_safe?`, and for
//! the same reason: without it, a helper that returns markup can only be spliced
//! by a tag that splices *anything*, so the one escape hatch has to serve both
//! "this is markup I built" and "this is a string I have decided to trust".
//! Those are different claims, and only the second is worth being nervous about.
//!
//! # How `{ … }` tells them apart
//!
//! [`Value`] has two impls: one for `Trusted`, which writes its markup through,
//! and a blanket over `Display`, which escapes. A `{ … }` calls
//! [`splice`], the value picks its own impl, and nothing about the
//! template says which.
//!
//! A blanket impl and a specific one normally overlap, because a downstream
//! crate could add the bound the specific type is missing. Here it cannot:
//! `Trusted` belongs to this crate and `Display` is `std`'s, so the impl that
//! would make them overlap is one nobody is allowed to write. `Trusted`
//! therefore does not implement `Display` — deliberately, and load bearing.
//! Markup is read back with [`Trusted::as_str`].

use crate::Renderer;
use crate::renderers::escape_html;
use std::borrow::Cow;
use std::fmt::Display;

/// A string that is markup, and is written into a document unescaped.
///
/// The invariant is the whole type: whatever is inside is already safe to
/// appear between tags. Every way of building one either escapes what it is
/// given ([`escaping`](Trusted::escaping), [`push`](Trusted::push)) or is an
/// explicit claim by the caller ([`from_markup`](Trusted::from_markup),
/// [`to_trusted`](ToTrusted::to_trusted)) — so the places worth auditing are
/// exactly the ones that name trust.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Trusted(String);

impl Trusted {
    /// An empty document.
    #[must_use]
    pub const fn new() -> Self {
        Trusted(String::new())
    }

    /// Take `markup` as markup, unescaped.
    ///
    /// This is the claim. It is `const` and free — no scan, no copy — because
    /// the point of the type is to make the claim once, where the markup is
    /// built, rather than at every place it is written.
    #[must_use]
    pub const fn from_markup(markup: String) -> Self {
        Trusted(markup)
    }

    /// Escape `value` into a new document.
    ///
    /// The inverse of [`from_markup`](Trusted::from_markup): this one is always
    /// safe, whatever the value came from.
    #[must_use]
    pub fn escaping(value: &dyn Display) -> Self {
        let mut out = Trusted::new();
        out.push_escaped(value);
        out
    }

    /// Borrow the markup.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Take the markup, giving up the claim that it is markup.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }

    /// Whether there is nothing to write.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The markup's length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Append `content`, escaping it if it is not already markup.
    ///
    /// This is [`SafeBuffer#<<`][rails]: a `&str` arrives escaped, another
    /// `Trusted` arrives whole, and neither call site has to remember which.
    ///
    /// [rails]: https://api.rubyonrails.org/classes/ActiveSupport/SafeBuffer.html
    pub fn push(&mut self, content: &impl Content) {
        content.write_content(self);
    }

    /// Append `markup` unescaped — the claim [`from_markup`](Trusted::from_markup)
    /// makes, for a document being built up.
    pub fn push_markup(&mut self, markup: &str) {
        self.0.push_str(markup);
    }

    /// Append `value`'s `Display` output, escaped.
    pub fn push_escaped(&mut self, value: &dyn Display) {
        // Formatting into a String is infallible, and the escape pass wants a
        // `&str` rather than the pieces `write!` would hand it one at a time.
        escape_html(&value.to_string(), &mut self.0);
    }
}

// `Trusted` implements neither `Display` nor `AsRef`-to-markup by accident: see
// [`Value`] for why the absence of `Display` is what makes `{ … }` able to tell
// markup from data at all.

impl AsRef<str> for Trusted {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Trusted> for String {
    fn from(trusted: Trusted) -> String {
        trusted.0
    }
}

impl crate::Render for Trusted {
    fn render_into(&self, r: &mut dyn Renderer) {
        r.write_raw(&self.0);
    }
}

/// A [`Renderer`] that appends to a [`Trusted`] under construction.
///
/// This is what lets a document being built by hand reach the attribute
/// machinery — [`Attr`](crate::Attr), [`ClassList`](crate::ClassList),
/// [`DataSet`](crate::DataSet) all write through a `Renderer`, and there is no
/// reason for [`tag!`](crate::tag) to own a second implementation of what
/// `disabled` or a class list means.
///
/// It escapes as HTML and lays nothing out, which is what a value being
/// assembled in Rust wants: there is no template whose newlines a policy could
/// be about.
pub struct Sink<'a>(&'a mut Trusted);

impl Renderer for Sink<'_> {
    fn write_raw(&mut self, s: &str) {
        self.0.push_markup(s);
    }

    fn write_escaped(&mut self, value: &dyn Display) {
        self.0.push_escaped(value);
    }

    /// The output is the [`Trusted`] this was borrowed from, which the caller
    /// still holds — so there is nothing to hand back here.
    fn finish(self: Box<Self>) -> String {
        String::new()
    }
}

impl Trusted {
    /// Borrow this document as a [`Renderer`], to write attributes into.
    #[must_use]
    pub fn sink(&mut self) -> Sink<'_> {
        Sink(self)
    }
}

/// Claim that a string is markup.
///
/// Named rather than blanket-implemented over `Display` on purpose: this is the
/// operation that can introduce a hole, so it should be spelled at the call
/// site — `user_input.to_trusted()` is a thing a reader can object to, and a
/// silent conversion is not.
pub trait ToTrusted {
    /// Take this string as markup, unescaped.
    fn to_trusted(&self) -> Trusted;
}

impl ToTrusted for str {
    fn to_trusted(&self) -> Trusted {
        Trusted(self.to_owned())
    }
}

impl ToTrusted for String {
    fn to_trusted(&self) -> Trusted {
        Trusted(self.clone())
    }
}

impl ToTrusted for Cow<'_, str> {
    fn to_trusted(&self) -> Trusted {
        Trusted(self.as_ref().to_owned())
    }
}

impl<T: ToTrusted + ?Sized> ToTrusted for &T {
    fn to_trusted(&self) -> Trusted {
        (**self).to_trusted()
    }
}

/// Something that can be the content of an element.
///
/// The escaping rule lives here rather than at the call sites: a `&str` child
/// is data and arrives escaped, a [`Trusted`] child is markup and arrives
/// whole. That is what lets `tag!(p, user.name)` be safe without the author
/// thinking about it, and `tag!(p, tag!(b, "x"))` be markup without a cast.
///
/// Unlike [`Value`], this one lists its types rather than taking a blanket over
/// `Display`, and the reason is the containers. rustc will accept a blanket
/// beside the `Trusted` impl — `Trusted` is local, so it can see nobody may add
/// the `Display` that would make them overlap — but it will not accept one
/// beside the impls for `Option`, `Vec` and the tuples, which are `std`'s and
/// could gain `Display` upstream. Children are where a list and a mixed tuple
/// are worth more than an arbitrary printable type, so the containers win here
/// and the blanket wins in `Value`.
///
/// Anything not listed reaches an element through `Trusted::escaping(&value)`,
/// or an impl of its own.
pub trait Content {
    /// Write this content into `out`, escaping it unless it is already markup.
    fn write_content(&self, out: &mut Trusted);
}

impl Content for Trusted {
    fn write_content(&self, out: &mut Trusted) {
        out.push_markup(&self.0);
    }
}

impl Content for str {
    fn write_content(&self, out: &mut Trusted) {
        escape_html(self, &mut out.0);
    }
}

impl Content for String {
    fn write_content(&self, out: &mut Trusted) {
        escape_html(self, &mut out.0);
    }
}

impl Content for Cow<'_, str> {
    fn write_content(&self, out: &mut Trusted) {
        escape_html(self, &mut out.0);
    }
}

/// Escaped through `Display`, like `{ … }` — a number or a `char` has nothing
/// to escape, but going through one path keeps the rule single.
macro_rules! content_via_display {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Content for $ty {
                fn write_content(&self, out: &mut Trusted) {
                    out.push_escaped(self);
                }
            }
        )*
    };
}

content_via_display!(
    bool, char, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64,
);

/// Nothing to write — the rule [`Render`](crate::Render) and
/// [`Attr`](crate::Attr) already follow, so absent content declines to appear
/// rather than appearing empty.
impl<T: Content> Content for Option<T> {
    fn write_content(&self, out: &mut Trusted) {
        if let Some(content) = self {
            content.write_content(out);
        }
    }
}

impl<T: Content + ?Sized> Content for &T {
    fn write_content(&self, out: &mut Trusted) {
        (**self).write_content(out);
    }
}

impl<T: Content> Content for [T] {
    fn write_content(&self, out: &mut Trusted) {
        for item in self {
            item.write_content(out);
        }
    }
}

impl<T: Content, const N: usize> Content for [T; N] {
    fn write_content(&self, out: &mut Trusted) {
        for item in self {
            item.write_content(out);
        }
    }
}

impl<T: Content> Content for Vec<T> {
    fn write_content(&self, out: &mut Trusted) {
        for item in self {
            item.write_content(out);
        }
    }
}

/// A tuple is how children of different types sit side by side with no boxing
/// and no common type — `(tag!(b, "x"), " and ", count)`.
macro_rules! content_for_tuples {
    ($(($($name:ident),+),)*) => {
        $(
            #[allow(non_snake_case)]
            impl<$($name: Content),+> Content for ($($name,)+) {
                fn write_content(&self, out: &mut Trusted) {
                    let ($($name,)+) = self;
                    $($name.write_content(out);)+
                }
            }
        )*
    };
}

content_for_tuples!(
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
);

/// What a `{ … }` writes, and how.
///
/// Two impls, and the pair is the whole mechanism: [`Trusted`] writes its
/// markup through untouched, and the blanket over `Display` escapes everything
/// else exactly as `{ … }` always has. So the value decides, rather than the
/// tag the template happened to use.
///
/// That these two can coexist rests on `Trusted` **not** implementing
/// `Display`. Coherence will not normally let a blanket impl sit beside a
/// specific one, because a downstream crate could add the missing bound later —
/// but `Trusted` is local to this crate and `Display` is foreign, so no other
/// crate is allowed to write that impl, and rustc can see the two do not
/// overlap. Adding `impl Display for Trusted` here would break this trait, and
/// that is the intended trade: markup is read back with
/// [`as_str`](Trusted::as_str) rather than through `format!`.
pub trait Value {
    /// Write this value into `r`, escaping it unless it is already markup.
    fn write_value(&self, r: &mut dyn Renderer);
}

impl<T: Display + ?Sized> Value for T {
    fn write_value(&self, r: &mut dyn Renderer) {
        r.write_escaped(&self);
    }
}

/// One impl per depth of reference, since `&Trusted` is not covered by the impl
/// for `Trusted` and is not `Display` either. Two covers a field and a binding
/// from an iterator over markup.
macro_rules! trusted_value {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Value for $ty {
                fn write_value(&self, r: &mut dyn Renderer) {
                    r.write_raw(self.as_str());
                }
            }
        )*
    };
}

trusted_value!(Trusted, &Trusted);

/// Write `value` into `r` — what every `{ … }` is lowered to.
///
/// A generic function rather than a bare method call for the reason
/// [`as_display`](crate::as_display) is one: the parameter stays a plain
/// `T: Value` bound, so a call site whose type is not yet settled — the
/// parameter of a `{#snippet}`, whose type arrives from wherever it is rendered
/// — infers as usual instead of forcing a resolution before there is anything
/// to resolve.
#[inline]
pub fn splice<T: Value + ?Sized>(value: &T, r: &mut dyn Renderer) {
    value.write_value(r);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HtmlRenderer;

    fn spliced<T: Value + ?Sized>(value: &T) -> String {
        let mut r = HtmlRenderer::new();
        splice(value, &mut r);
        r.as_str().to_owned()
    }

    #[test]
    fn markup_is_spliced_whole() {
        assert_eq!(
            spliced(&Trusted::from_markup("<b>x</b>".into())),
            "<b>x</b>"
        );
    }

    #[test]
    fn a_reference_to_markup_is_still_markup() {
        let markup = Trusted::from_markup("<i>x</i>".into());
        assert_eq!(spliced(&&markup), "<i>x</i>");
    }

    #[test]
    fn escaping_is_the_inverse_of_a_claim() {
        assert_eq!(Trusted::escaping(&"<b>").as_str(), "&lt;b&gt;");
        assert_eq!("<b>".to_trusted().as_str(), "<b>");
    }

    #[test]
    fn pushing_escapes_data_and_keeps_markup() {
        let mut out = Trusted::new();
        out.push(&"a<b");
        out.push(&Trusted::from_markup("<br>".into()));
        out.push(&Some("c&d"));
        out.push(&None::<&str>);
        assert_eq!(out.as_str(), "a&lt;b<br>c&amp;d");
    }

    #[test]
    fn a_tuple_writes_its_parts_in_order() {
        let mut out = Trusted::new();
        out.push(&(Trusted::from_markup("<b>".into()), "x<", 1_u8));
        assert_eq!(out.as_str(), "<b>x&lt;1");
    }

    #[test]
    fn a_list_writes_every_item() {
        let mut out = Trusted::new();
        out.push(&vec!["a", "<b>"]);
        assert_eq!(out.as_str(), "a&lt;b&gt;");
    }

    /// The absence of `Display` is what makes markup in an untyped snippet a
    /// compile error rather than an escaped mess on the page.
    #[test]
    fn markup_is_read_back_as_a_string_explicitly() {
        let markup = Trusted::from_markup("<b>x</b>".into());
        assert_eq!(markup.as_str(), "<b>x</b>");
        assert_eq!(markup.into_string(), "<b>x</b>");
    }
}
