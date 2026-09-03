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

    /// The same, minus any attribute the element already writes itself.
    ///
    /// An element's own attributes win, and the spread fills in the rest. The
    /// alternative is a tag carrying `type` twice, which is not valid HTML and
    /// which the browser resolves by a rule nobody writing the template was
    /// thinking about.
    ///
    /// `taken` is decided when the template compiles — it is the literal
    /// attribute names in that tag — so filtering costs a short scan of a list
    /// that is usually empty and never long.
    ///
    /// The default ignores it, which is the honest answer for a spread that is
    /// markup rather than pairs: `&'static str` has no names to compare, so
    /// there is nothing it could drop.
    fn write_attrs_except(&self, taken: &[&str], r: &mut dyn Renderer) {
        let _ = taken;
        self.write_attrs(r);
    }
}

impl<T: AttrSpread + ?Sized> AttrSpread for &T {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        (**self).write_attrs(r);
    }

    fn write_attrs_except(&self, taken: &[&str], r: &mut dyn Renderer) {
        (**self).write_attrs_except(taken, r);
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

    fn write_attrs_except(&self, taken: &[&str], r: &mut dyn Renderer) {
        if let Some(inner) = self {
            inner.write_attrs_except(taken, r);
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
        self.write_attrs_except(&[], r);
    }

    fn write_attrs_except(&self, taken: &[&str], r: &mut dyn Renderer) {
        for (key, value) in self {
            let key = key.as_ref();
            if !is_attr_name_safe(key) {
                debug_assert!(false, "`{key}` is not usable as an attribute name");
                continue;
            }
            if taken.contains(&key) {
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

    fn write_attrs_except(&self, taken: &[&str], r: &mut dyn Renderer) {
        self.as_slice().write_attrs_except(taken, r);
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

    fn write_attrs_except(&self, taken: &[&str], r: &mut dyn Renderer) {
        for (key, value) in &self.entries {
            if taken
                .iter()
                .any(|name| name.strip_prefix("data-") == Some(key.as_str()))
            {
                continue;
            }
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

/// What one attribute holds — the three outcomes [`Attr`] already draws,
/// named so that they can be *stored* rather than written straight out.
///
/// [`Attrs`] is a bag a component carries around before it knows where the
/// attributes are going, so the decision a `bool` or an `Option` makes has to
/// survive being put down and picked up again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    /// Not written at all — what `None` and `false` mean.
    Absent,
    /// Written bare, with no `="…"` — what `true` means.
    Bare,
    /// Written as ` name="value"`, escaped.
    Value(Cow<'static, str>),
}

/// A value that can be put into an [`Attrs`].
///
/// The same set [`Attr`] covers, and deliberately no blanket impl over
/// `Display`, for the same reason: `bool` and `Option` decide *whether* an
/// attribute appears, and a blanket impl would collide with them and turn
/// `disabled={false}` into a disabled control.
pub trait IntoAttrValue {
    fn into_attr_value(self) -> AttrValue;
}

impl IntoAttrValue for AttrValue {
    fn into_attr_value(self) -> AttrValue {
        self
    }
}

/// `true` is the bare attribute, `false` is no attribute — the HTML rule.
impl IntoAttrValue for bool {
    fn into_attr_value(self) -> AttrValue {
        match self {
            true => AttrValue::Bare,
            false => AttrValue::Absent,
        }
    }
}

impl<T: IntoAttrValue> IntoAttrValue for Option<T> {
    fn into_attr_value(self) -> AttrValue {
        match self {
            Some(value) => value.into_attr_value(),
            None => AttrValue::Absent,
        }
    }
}

impl IntoAttrValue for String {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Value(Cow::Owned(self))
    }
}

impl IntoAttrValue for Cow<'static, str> {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Value(self)
    }
}

/// Borrowed for however long the value lives, so it is copied. Static text
/// written in a template does not come through here — see
/// [`Attrs::insert_static`], which the lowering emits for a quoted value and
/// which keeps it borrowed.
impl IntoAttrValue for &str {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Value(Cow::Owned(self.to_string()))
    }
}

impl IntoAttrValue for &String {
    fn into_attr_value(self) -> AttrValue {
        AttrValue::Value(Cow::Owned(self.clone()))
    }
}

macro_rules! attr_value_via_display {
    ($($ty:ty),* $(,)?) => {$(
        impl IntoAttrValue for $ty {
            fn into_attr_value(self) -> AttrValue {
                AttrValue::Value(Cow::Owned(self.to_string()))
            }
        }
    )*};
}

attr_value_via_display!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
);

/// The attributes a component was given that it does not name.
///
/// A component declares one field for them — `#[prop(rest)] pub attrs: Attrs` —
/// and every attribute at its call site that is not one of its props lands here
/// instead of failing to compile. The component decides where they go by
/// spreading the field onto an element of its markup:
///
/// ```html
/// <input type="hidden" name={self.name()} {...self.attrs}>
/// ```
///
/// So `<Hidden data-cover-target="input"/>` reaches the page without the
/// component having heard of `data-cover-target`, and without the call site
/// hand-writing markup.
///
/// # What is in it
///
/// Name/value pairs, and only pairs. They are keyed the way [`DataSet`] is: a
/// name given twice keeps its **first position** and takes its **last value**,
/// so a component that fills in a default can be overridden without the output
/// reshuffling. A name that could not be written safely is dropped; see
/// [`is_attr_name_safe`].
///
/// There is deliberately no way to put *markup* in here. Attributes assembled
/// as a string were the older `attrs={r#"…"#}` spelling, and every one of them
/// is now written at the call site as the attributes it always was — which is
/// the point of the bag, and which lets each name and value be escaped as one.
#[derive(Debug, Default, Clone)]
pub struct Attrs {
    /// `None` is a bare attribute, kept distinct from `Some("")` for the same
    /// reason [`Attr`] keeps it: `checked` and `checked=""` are both checked,
    /// but [`Attrs::get`] should not have to guess which was meant.
    entries: Vec<(Cow<'static, str>, Option<Cow<'static, str>>)>,
}

impl Attrs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `name`, whatever [`IntoAttrValue`] says the value is — including
    /// saying there is no attribute at all, which removes it.
    pub fn insert(&mut self, name: impl Into<Cow<'static, str>>, value: impl IntoAttrValue) {
        let name = name.into();
        match value.into_attr_value() {
            AttrValue::Absent => self.remove(&name),
            AttrValue::Bare => self.put(name, None),
            AttrValue::Value(value) => self.put(name, Some(value)),
        }
    }

    /// Sets `name` to static text, keeping it borrowed. What the lowering emits
    /// for `data-cover-target="input"`, where both halves are template source.
    pub fn insert_static(&mut self, name: &'static str, value: &'static str) {
        self.put(Cow::Borrowed(name), Some(Cow::Borrowed(value)));
    }

    /// Sets `name` with no value, written as a bare ` name`.
    pub fn insert_bare(&mut self, name: impl Into<Cow<'static, str>>) {
        self.put(name.into(), None);
    }

    /// Drops `name`, whatever set it.
    pub fn remove(&mut self, name: &str) {
        self.entries.retain(|(seen, _)| seen != name);
    }

    /// The value `name` holds, or `None` when it is unset *or* bare. Use
    /// [`Attrs::contains`] to tell those two apart.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(seen, _)| seen == name)
            .and_then(|(_, value)| value.as_deref())
    }

    /// Whether `name` is set at all, bare included.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|(seen, _)| seen == name)
    }

    /// Every pair, in the order it will be written. A bare attribute yields
    /// `None`.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.entries
            .iter()
            .map(|(name, value)| (name.as_ref(), value.as_deref()))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Folds a set in, as though its entries had been added to this one in
    /// order: a name already here keeps its position and takes the new value.
    pub fn merge(&mut self, other: &(impl AttrSet + ?Sized)) {
        other.add_attrs(self);
    }

    fn put(&mut self, name: Cow<'static, str>, value: Option<Cow<'static, str>>) {
        if !is_attr_name_safe(&name) {
            debug_assert!(false, "`{name}` is not usable as an attribute name");
            return;
        }
        match self.entries.iter_mut().find(|(seen, _)| *seen == name) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((name, value)),
        }
    }
}

/// How the bag reaches the page: `{...self.attrs}` on any element.
impl AttrSpread for Attrs {
    fn write_attrs(&self, r: &mut dyn Renderer) {
        self.write_attrs_except(&[], r);
    }

    fn write_attrs_except(&self, taken: &[&str], r: &mut dyn Renderer) {
        for (name, value) in &self.entries {
            if taken.contains(&name.as_ref()) {
                continue;
            }
            r.write_raw(" ");
            r.write_escaped(&name.as_ref());
            if let Some(value) = value {
                r.write_raw("=\"");
                r.write_escaped(&value.as_ref());
                r.write_raw("\"");
            }
        }
    }
}

/// Something that can contribute whole entries to an [`Attrs`].
///
/// The seam a `#[prop(rest)]` field is filled through: `attrs={…}` written at a
/// call site, and `{...expr}` spread onto a component. The same shape as
/// [`DataItem`], and for the same reason — a set is read from a reference and
/// folded in, rather than moved out of the component holding it.
///
/// Notably *not* implemented for strings. A string is markup, and this bag
/// holds pairs it escapes one at a time; the diagnostic below is what the older
/// `attrs={r#"data-controller="signup""#}` spelling now says, so a call site
/// that still assembles markup is a build failure rather than a page that looks
/// right until a value in it needs escaping.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a set of attributes",
    label = "expected `Attrs`, a list of name/value pairs, or an `Option` of either",
    note = "attributes written as raw markup are no longer accepted — write them at the call site as attributes: `<Hidden data-controller=\"signup\"/>`"
)]
pub trait AttrSet {
    fn add_attrs(&self, attrs: &mut Attrs);
}

impl<T: AttrSet + ?Sized> AttrSet for &T {
    fn add_attrs(&self, attrs: &mut Attrs) {
        (**self).add_attrs(attrs);
    }
}

impl<T: AttrSet> AttrSet for Option<T> {
    fn add_attrs(&self, attrs: &mut Attrs) {
        if let Some(set) = self {
            set.add_attrs(attrs);
        }
    }
}

impl AttrSet for Attrs {
    fn add_attrs(&self, attrs: &mut Attrs) {
        for (name, value) in &self.entries {
            attrs.put(name.clone(), value.clone());
        }
    }
}

impl<K: AsRef<str>, V: IntoAttrValue + Clone> AttrSet for [(K, V)] {
    fn add_attrs(&self, attrs: &mut Attrs) {
        for (name, value) in self {
            attrs.insert(name.as_ref().to_string(), value.clone());
        }
    }
}

impl<K: AsRef<str>, V: IntoAttrValue + Clone, const N: usize> AttrSet for [(K, V); N] {
    fn add_attrs(&self, attrs: &mut Attrs) {
        self.as_slice().add_attrs(attrs);
    }
}

impl<K: AsRef<str>, V: IntoAttrValue + Clone> AttrSet for Vec<(K, V)> {
    fn add_attrs(&self, attrs: &mut Attrs) {
        self.as_slice().add_attrs(attrs);
    }
}

impl<K: Into<Cow<'static, str>>, V: IntoAttrValue> FromIterator<(K, V)> for Attrs {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(pairs: I) -> Self {
        let mut attrs = Attrs::new();
        for (name, value) in pairs {
            attrs.insert(name, value);
        }
        attrs
    }
}

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
