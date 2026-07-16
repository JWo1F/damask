//! What an attribute value may be, and how a class list is built.
//!
//! Two seams live here. [`Attr`] decides how `name={expr}` reaches the output —
//! including whether it reaches it at all, which is how a `bool` renders a bare
//! `disabled` and an `Option` renders nothing. [`ClassList`] backs the richer
//! `class` forms, where the value is a set of names assembled from parts rather
//! than one string.

use crate::Renderer;
use std::borrow::Cow;

/// A value that knows how to render itself as an attribute — or not to.
///
/// Implemented for the string types, the numbers, `bool`, and `Option` of any
/// of them. There is deliberately no blanket impl over [`core::fmt::Display`]:
/// it would collide with the `bool` and `Option` impls, which are the whole
/// point, since `disabled="false"` is a *disabled* control and an absent
/// attribute is the only way to say otherwise. A type of your own either
/// implements this trait or reaches the template as a string.
pub trait Attr {
    /// Write ` name="value"`, a bare ` name`, or nothing at all.
    fn write_attr(&self, name: &str, r: &mut dyn Renderer);
}

/// Writes ` name="value"`, escaping the value.
fn write_pair(name: &str, value: &dyn core::fmt::Display, r: &mut dyn Renderer) {
    r.write_raw(" ");
    r.write_raw(name);
    r.write_raw("=\"");
    r.write_escaped(value);
    r.write_raw("\"");
}

/// A bare boolean attribute: present when true, absent when false.
///
/// This is the HTML rule, not a convenience — the presence of `disabled` is
/// what disables a control, and every value it could carry, `"false"`
/// included, leaves it disabled.
impl Attr for bool {
    fn write_attr(&self, name: &str, r: &mut dyn Renderer) {
        if *self {
            r.write_raw(" ");
            r.write_raw(name);
        }
    }
}

/// `None` omits the attribute entirely.
impl<T: Attr> Attr for Option<T> {
    fn write_attr(&self, name: &str, r: &mut dyn Renderer) {
        if let Some(value) = self {
            value.write_attr(name, r);
        }
    }
}

impl<T: Attr + ?Sized> Attr for &T {
    fn write_attr(&self, name: &str, r: &mut dyn Renderer) {
        (**self).write_attr(name, r);
    }
}

impl Attr for str {
    fn write_attr(&self, name: &str, r: &mut dyn Renderer) {
        write_pair(name, &self, r);
    }
}

impl Attr for String {
    fn write_attr(&self, name: &str, r: &mut dyn Renderer) {
        write_pair(name, &self.as_str(), r);
    }
}

impl Attr for Cow<'_, str> {
    fn write_attr(&self, name: &str, r: &mut dyn Renderer) {
        write_pair(name, &self.as_ref(), r);
    }
}

macro_rules! attr_via_display {
    ($($t:ty),* $(,)?) => {$(
        impl Attr for $t {
            fn write_attr(&self, name: &str, r: &mut dyn Renderer) {
                write_pair(name, self, r);
            }
        }
    )*};
}

attr_via_display!(
    char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);

/// A run of attributes spliced into a tag by `{...expr}`.
///
/// For the attributes a component cannot name: those whose *name* is computed
/// (`data-<controller>-target`) or that arrive as a map. An implementor writes
/// its own attribute text, so it owns its own escaping — which is why this is
/// implemented for a key/value pair list, which escapes, and for `&'static str`,
/// whose lifetime is the guarantee that it is markup the author wrote rather
/// than a value that reached the page from a request.
pub trait AttrSpread {
    fn write_attrs(&self, r: &mut dyn Renderer);
}

impl<T: AttrSpread + ?Sized> AttrSpread for &T {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        (**self).write_attrs(r);
    }
}

/// Markup written by the author. `&'static str` and not `String`: a value that
/// came from config or a request cannot be `'static`, so it cannot arrive here.
impl AttrSpread for &'static str {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        if !self.is_empty() {
            r.write_raw(" ");
            r.write_raw(self);
        }
    }
}

impl<T: AttrSpread> AttrSpread for Option<T> {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        if let Some(inner) = self {
            inner.write_attrs(r);
        }
    }
}

/// Name/value pairs, escaped. The form to use for anything derived from state.
impl<K: AsRef<str>, V: AsRef<str>> AttrSpread for [(K, V)] {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        for (key, value) in self {
            r.write_raw(" ");
            r.write_escaped(&key.as_ref());
            r.write_raw("=\"");
            r.write_escaped(&value.as_ref());
            r.write_raw("\"");
        }
    }
}

impl<K: AsRef<str>, V: AsRef<str>> AttrSpread for Vec<(K, V)> {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        self.as_slice().write_attrs(r);
    }
}

/// A set of class names, assembled then written once.
///
/// Ordered by first mention and deduplicated, which is what makes the `class:`
/// directives able to override the base list: adding a name already present is
/// a no-op, and removing one removes it wherever it came from.
#[derive(Debug, Default, Clone)]
pub struct ClassList {
    names: Vec<String>,
}

impl ClassList {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds every whitespace-separated name in `text`.
    ///
    /// Splitting here rather than at the call site is what lets a single
    /// expression contribute a run of classes — the common case, since a tone
    /// or a variant resolves to several at once.
    pub fn add(&mut self, text: &str) {
        for name in text.split_whitespace() {
            if !self.names.iter().any(|n| n == name) {
                self.names.push(name.to_string());
            }
        }
    }

    /// Adds or removes `text`, per a `class:name={cond}` directive.
    pub fn set(&mut self, text: &str, on: bool) {
        if on {
            self.add(text);
        } else {
            for name in text.split_whitespace() {
                self.names.retain(|n| n != name);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn to_value(&self) -> String {
        self.names.join(" ")
    }

    /// Writes ` class="…"`, or nothing when the list came out empty — an empty
    /// `class` attribute says nothing that its absence does not.
    pub fn write_attr(&self, name: &str, r: &mut dyn Renderer) {
        if !self.is_empty() {
            write_pair(name, &self.to_value(), r);
        }
    }
}

/// Something that can contribute to a [`ClassList`].
///
/// The `Option` impl is why `[Some("a"), None, "b"]` type-checks item by item:
/// each entry is lowered to its own call, so the items need no common type.
pub trait ClassItem {
    fn add_to(&self, list: &mut ClassList);
}

impl<T: ClassItem + ?Sized> ClassItem for &T {
    fn add_to(&self, list: &mut ClassList) {
        (**self).add_to(list);
    }
}

impl<T: ClassItem> ClassItem for Option<T> {
    fn add_to(&self, list: &mut ClassList) {
        if let Some(item) = self {
            item.add_to(list);
        }
    }
}

impl ClassItem for str {
    fn add_to(&self, list: &mut ClassList) {
        list.add(self);
    }
}

impl ClassItem for String {
    fn add_to(&self, list: &mut ClassList) {
        list.add(self);
    }
}

impl ClassItem for Cow<'_, str> {
    fn add_to(&self, list: &mut ClassList) {
        list.add(self.as_ref());
    }
}
