//! What an attribute value may be, and how a class list or data set is built.
//!
//! Three seams live here. [`Attr`] decides how `name={expr}` reaches the output —
//! including whether it reaches it at all, which is how a `bool` renders a bare
//! `disabled` and an `Option` renders nothing. [`ClassList`] backs the richer
//! `class` forms, where the value is a set of names assembled from parts rather
//! than one string. [`DataSet`] backs the `data` forms, where one value expands
//! into a run of `data-*` attributes.

use crate::Renderer;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

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

/// Whether `name` is safe to write as an attribute name.
///
/// Escaping is a value's defence and cannot be a name's: a space or an `=`
/// inside a name does not need escaping to be dangerous, it simply ends the
/// name and begins a second attribute — so a key that arrived from state could
/// otherwise smuggle in an `onclick`. Names that could do that are refused
/// rather than escaped. The refused set is HTML's own — control characters,
/// whitespace, and `" ' > / =` — widened by `<` and `&`, which no name a
/// template should be writing needs.
///
/// This is the check [`DataSet`] and the key/value [`AttrSpread`] apply to
/// every key they are given; it is public so that an `AttrSpread` of your own
/// can apply the same one.
pub fn is_attr_name_safe(name: &str) -> bool {
    !name.is_empty()
        && !name.chars().any(|c| {
            c.is_control()
                || c.is_whitespace()
                || matches!(c, '"' | '\'' | '>' | '<' | '/' | '=' | '&')
        })
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
///
/// A pair whose name would not survive being written as one — see
/// [`is_attr_name_safe`] — is dropped rather than emitted, because escaping the
/// name cannot make it safe. That is a `debug_assert` in a debug build, so the
/// mistake is loud where it can be fixed and harmless where it cannot.
impl<K: AsRef<str>, V: AsRef<str>> AttrSpread for [(K, V)] {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        for (key, value) in self {
            let key = key.as_ref();
            if !is_attr_name_safe(key) {
                debug_assert!(false, "`{key}` is not usable as an attribute name");
                continue;
            }
            r.write_raw(" ");
            r.write_escaped(&key);
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

/// A set of `data-*` attributes, assembled then written once.
///
/// Keyed rather than ordered like [`ClassList`], because a data attribute
/// carries a value: a key mentioned twice keeps the **first position** it was
/// given and takes the **last value**, which is what lets `data=[base, extra]`
/// mean that `extra` overrides `base` without reshuffling the output.
///
/// A key is the part *after* `data-` — `"controller"` writes
/// `data-controller` — and is never rewritten on the way out, so
/// `"user_id"` stays `data-user_id`. One that could not be written safely is
/// dropped; see [`is_attr_name_safe`].
#[derive(Debug, Default, Clone)]
pub struct DataSet {
    /// `None` is a bare attribute — the same distinction [`Attr`] draws for
    /// `bool`, kept here rather than collapsed to a `"true"` string so that the
    /// two forms cannot drift apart.
    entries: Vec<(String, Option<String>)>,
}

impl DataSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `key` to `value`, written as ` data-key="value"`.
    pub fn insert(&mut self, key: &str, value: impl Into<String>) {
        self.put(key, Some(value.into()));
    }

    /// Sets `key` with no value, written as a bare ` data-key`.
    pub fn insert_bare(&mut self, key: &str) {
        self.put(key, None);
    }

    /// Drops `key`, whatever set it.
    pub fn remove(&mut self, key: &str) {
        self.entries.retain(|(seen, _)| seen != key);
    }

    fn put(&mut self, key: &str, value: Option<String>) {
        if !is_attr_name_safe(key) {
            debug_assert!(false, "`{key}` is not usable as a data attribute name");
            return;
        }
        match self.entries.iter_mut().find(|(seen, _)| seen == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key.to_string(), value)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Writes every entry as ` data-key="value"`, or a bare ` data-key`.
    pub fn write_attrs(&self, r: &mut dyn Renderer) {
        for (key, value) in &self.entries {
            r.write_raw(" data-");
            r.write_escaped(&key.as_str());
            if let Some(value) = value {
                r.write_raw("=\"");
                r.write_escaped(&value.as_str());
                r.write_raw("\"");
            }
        }
    }
}

/// So a set built in Rust can also be spliced with `{...expr}`.
impl AttrSpread for DataSet {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        DataSet::write_attrs(self, r);
    }
}

/// Something that can contribute whole entries to a [`DataSet`].
///
/// This is the seam that makes `data={expr}` broad: a map, an ordered pair
/// list, an `Option` of either, or a type of your own that knows which
/// attributes it wants. Each entry of a `data=[…]` list is lowered to its own
/// `add_to` call, so entries need no common type.
///
/// There is deliberately no impl for a bare string. A string carries names but
/// no values, which is what a class list is made of; a data set is made of
/// pairs, and there is no reading of `"a b"` as one that would not be a guess.
pub trait DataItem {
    fn add_to(&self, set: &mut DataSet);
}

impl<T: DataItem + ?Sized> DataItem for &T {
    fn add_to(&self, set: &mut DataSet) {
        (**self).add_to(set);
    }
}

impl<T: DataItem> DataItem for Option<T> {
    fn add_to(&self, set: &mut DataSet) {
        if let Some(item) = self {
            item.add_to(set);
        }
    }
}

impl DataItem for DataSet {
    fn add_to(&self, set: &mut DataSet) {
        for (key, value) in &self.entries {
            set.put(key, value.clone());
        }
    }
}

impl<K: AsRef<str>, V: DataValue> DataItem for [(K, V)] {
    fn add_to(&self, set: &mut DataSet) {
        for (key, value) in self {
            value.add_to(key.as_ref(), set);
        }
    }
}

impl<K: AsRef<str>, V: DataValue, const N: usize> DataItem for [(K, V); N] {
    fn add_to(&self, set: &mut DataSet) {
        self.as_slice().add_to(set);
    }
}

impl<K: AsRef<str>, V: DataValue> DataItem for Vec<(K, V)> {
    fn add_to(&self, set: &mut DataSet) {
        self.as_slice().add_to(set);
    }
}

impl<K: AsRef<str>, V: DataValue> DataItem for BTreeMap<K, V> {
    fn add_to(&self, set: &mut DataSet) {
        for (key, value) in self {
            value.add_to(key.as_ref(), set);
        }
    }
}

/// Visited in key order rather than the map's own, because a `HashMap` has no
/// stable one — the same render would otherwise emit the same attributes in a
/// different order each run, which no snapshot test or cache could live with.
impl<K: AsRef<str>, V: DataValue, S> DataItem for HashMap<K, V, S> {
    fn add_to(&self, set: &mut DataSet) {
        let mut pairs: Vec<(&str, &V)> = self.iter().map(|(k, v)| (k.as_ref(), v)).collect();
        pairs.sort_by_key(|(key, _)| *key);
        for (key, value) in pairs {
            value.add_to(key, set);
        }
    }
}

/// How one value in a data set appears — or declines to.
///
/// The mirror of [`Attr`], one level down: where `Attr` decides how a whole
/// attribute reaches the tag, this decides how a single entry reaches the set,
/// and it answers the two questions the same way. A `bool` renders a bare
/// `data-open` when true and nothing when false, and an `Option` renders
/// nothing when `None`. It is a separate trait only because a set has to be
/// assembled before it is written, and `Attr` writes as it goes.
pub trait DataValue {
    fn add_to(&self, key: &str, set: &mut DataSet);
}

impl<T: DataValue + ?Sized> DataValue for &T {
    fn add_to(&self, key: &str, set: &mut DataSet) {
        (**self).add_to(key, set);
    }
}

impl<T: DataValue> DataValue for Option<T> {
    fn add_to(&self, key: &str, set: &mut DataSet) {
        if let Some(value) = self {
            value.add_to(key, set);
        }
    }
}

/// A bare data attribute: present when true, absent when false.
///
/// The same rule as [`Attr for bool`](Attr#impl-Attr-for-bool), so a flag means
/// one thing across the whole template language. Note that a bare `data-open`
/// reaches JavaScript as `el.dataset.open === ""`, which is falsy — read
/// presence with `in`, or carry `"true"` as a string when a script wants to
/// test the value.
impl DataValue for bool {
    fn add_to(&self, key: &str, set: &mut DataSet) {
        if *self {
            set.insert_bare(key);
        }
    }
}

impl DataValue for str {
    fn add_to(&self, key: &str, set: &mut DataSet) {
        set.insert(key, self);
    }
}

impl DataValue for String {
    fn add_to(&self, key: &str, set: &mut DataSet) {
        set.insert(key, self.as_str());
    }
}

impl DataValue for Cow<'_, str> {
    fn add_to(&self, key: &str, set: &mut DataSet) {
        set.insert(key, self.as_ref());
    }
}

macro_rules! data_value_via_display {
    ($($t:ty),* $(,)?) => {$(
        impl DataValue for $t {
            fn add_to(&self, key: &str, set: &mut DataSet) {
                set.insert(key, self.to_string());
            }
        }
    )*};
}

data_value_via_display!(
    char, u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderers::{StringRenderer, escape_html};

    fn write(f: impl FnOnce(&mut dyn Renderer)) -> String {
        let mut r = StringRenderer::with_escape(escape_html);
        f(&mut r);
        Box::new(r).finish()
    }

    fn rendered(item: &dyn DataItem) -> String {
        let mut set = DataSet::new();
        item.add_to(&mut set);
        write(|r| set.write_attrs(r))
    }

    #[test]
    fn pairs_and_maps_reach_the_same_attributes() {
        let want = r#" data-controller="modal" data-index="3""#;
        assert_eq!(rendered(&[("controller", "modal"), ("index", "3")]), want);
        assert_eq!(
            rendered(&vec![("controller", "modal"), ("index", "3")]),
            want
        );
        let tree: BTreeMap<&str, &str> = [("controller", "modal"), ("index", "3")]
            .into_iter()
            .collect();
        assert_eq!(rendered(&tree), want);
    }

    /// A `HashMap` has no order of its own, so the set imposes one — otherwise
    /// the same state would render differently from run to run.
    #[test]
    fn a_hash_map_renders_in_key_order() {
        let map: HashMap<&str, &str> = [("b", "2"), ("a", "1"), ("c", "3")].into_iter().collect();
        assert_eq!(rendered(&map), r#" data-a="1" data-b="2" data-c="3""#);
    }

    /// The value decides whether its attribute appears at all, exactly as
    /// [`Attr`] does one level up.
    #[test]
    fn a_value_may_decline_to_appear() {
        assert_eq!(rendered(&[("open", true)]), " data-open");
        assert_eq!(rendered(&[("open", false)]), "");
        assert_eq!(rendered(&[("note", Some("hi"))]), r#" data-note="hi""#);
        assert_eq!(rendered(&[("note", None::<&str>)]), "");
        // A whole item may be absent, not just one value.
        assert_eq!(rendered(&None::<[(&str, &str); 1]>), "");
    }

    #[test]
    fn numbers_and_chars_carry_their_display() {
        assert_eq!(rendered(&[("count", 42u32)]), r#" data-count="42""#);
        assert_eq!(rendered(&[("sep", ',')]), r#" data-sep=",""#);
    }

    /// The rule that makes `data=[base, extra]` mean "extra overrides base":
    /// the later value wins, and the key stays where it was first mentioned.
    #[test]
    fn a_repeated_key_keeps_its_place_and_takes_the_last_value() {
        let mut set = DataSet::new();
        [("a", "1"), ("b", "2")].add_to(&mut set);
        [("a", "9")].add_to(&mut set);
        assert_eq!(write(|r| set.write_attrs(r)), r#" data-a="9" data-b="2""#);
    }

    #[test]
    fn keys_are_written_verbatim_and_values_escaped() {
        assert_eq!(
            rendered(&[("user_id", "a\"b<c")]),
            r#" data-user_id="a&quot;b&lt;c""#
        );
    }

    #[test]
    fn a_set_can_be_spread_and_folded_into_another() {
        let mut base = DataSet::new();
        base.insert("controller", "modal");
        base.insert_bare("open");
        assert_eq!(
            write(|r| AttrSpread::write_attrs(&base, r)),
            r#" data-controller="modal" data-open"#
        );

        let mut set = DataSet::new();
        base.add_to(&mut set);
        set.remove("open");
        assert_eq!(write(|r| set.write_attrs(r)), r#" data-controller="modal""#);
    }

    /// Escaping is a value's defence and cannot be a name's — a space in a key
    /// would end the name and begin a second attribute, so the key is refused.
    #[test]
    fn a_name_that_could_break_out_of_its_attribute_is_refused() {
        for bad in ["", "x onclick", "x=y", "a\"b", "a>b", "a/b", "a&b", "a\tb"] {
            assert!(!is_attr_name_safe(bad), "`{bad}` must not be usable");
        }
        for good in ["controller", "user_id", "modal-open", "x:y", "п"] {
            assert!(is_attr_name_safe(good), "`{good}` must be usable");
        }
    }

    /// The same guard on the key/value spread, which had escaped the name but
    /// escaping is not what a name needs.
    #[test]
    #[cfg(not(debug_assertions))]
    fn a_spread_drops_a_name_it_cannot_write() {
        let pairs = [("safe", "1"), ("x onclick", "alert(1)")];
        assert_eq!(write(|r| pairs.write_attrs(r)), r#" safe="1""#);
    }
}
