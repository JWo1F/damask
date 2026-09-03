//! # Damask — compile-time components for Rust
//!
//! React-like, compile-time components for Rust. A component is a struct (its
//! fields are its props) paired with a `.dmk` template. The [`Component`] derive
//! compiles the template into a [`Render::render_into`] method, so rendering
//! is plain Rust with no runtime template parsing.
//!
//! ```ignore
//! use damask::Component;
//!
//! // greeting.rs  — paired with greeting.dmk containing: Hello {self.name}!
//! #[derive(Component)]
//! pub struct Greeting {
//!     pub name: String,
//! }
//!
//! let out = Greeting { name: "Ada".into() }.render();
//! assert_eq!(out, "Hello Ada!");
//! ```
//!
//! ## The two traits
//!
//! - [`Renderer`] is the extensibility seam: it owns the output buffer and the
//!   escaping policy. Implement it to change escaping, target a different sink,
//!   or stream. Components render into `&mut dyn Renderer`, so a component
//!   compiled once works with any renderer.
//! - [`Component`] is implemented by the macro. [`Component::render`] renders to
//!   a `String` using the default renderer chosen by the template's host
//!   language.
//!
//! See [`renderers`] for the built-ins and [`StringRenderer`](renderers::StringRenderer)
//! for the easiest custom-renderer starting point.
//!
//! ## Props and slots
//!
//! A struct's fields are its props. Its template's `<slot>`s are *not* fields:
//! they are content the caller supplies, and they travel as a [`Slots`] argument
//! to [`Render::render_slots`]. See [`Slots`] for what that buys and costs — and
//! for the methods a template reaches through its `slots` binding, which is that
//! same argument under a name templates may write.
//!
//! A prop must be passed unless its type says what leaving it out means:
//! `Option<_>` is `None`. `#[component(default)]` on the struct extends that to
//! every prop, filling the skipped ones from its `Default`. See [`props`] for
//! how a call site — which cannot see the struct it is building — is held to
//! that.

use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;

pub mod attr;
pub mod props;
pub mod renderers;
pub mod trusted;

pub use attr::{
    Attr, AttrSet, AttrSpread, AttrValue, Attrs, IntoAttrValue, TokenItem, TokenList,
    is_attr_name_safe,
};
pub use renderers::{HtmlRenderer, Whitespace};
pub use trusted::{Content, Sink, ToTrusted, Trusted, Value, splice};

/// Derive macro that generates a [`Component`] impl from a struct's paired
/// `.dmk` template. Shares its name with the trait (like `serde::Serialize`), so
/// `use damask::Component;` brings both into scope.
pub use damask_macros::Component;

/// Implementation detail of [`tag!`](crate::tag); not to be invoked directly.
#[doc(hidden)]
pub use damask_macros::__tag;

/// Build one element as [`Trusted`] markup, in Rust.
///
/// The counterpart to a `.dmk` for the markup a template is the wrong shape
/// for: a helper in a service, a fragment a handler assembles, a `<style>`
/// element whose stylesheet is full of the `{` a template reserves.
///
/// ```
/// use damask::tag;
///
/// let flagged = true;
/// let markup = tag!(div #summary, class: @tokens("card", flagged.then_some("is-flagged")), {
///     (
///         tag!(span, "Total"),
///         tag!(b, "1 < 2"),
///     )
/// });
/// assert_eq!(
///     markup.as_str(),
///     r#"<div id="summary" class="card is-flagged"><span>Total</span><b>1 &lt; 2</b></div>"#
/// );
/// ```
///
/// The element comes first. After it come `name: value` attributes in any
/// order, and last, with no name, what goes inside. A `&str` child is escaped
/// and a [`Trusted`] one is spliced, so `tag!(p, user_name)` is safe and
/// `tag!(p, tag!(b, "x"))` is markup, neither of them by saying so.
///
/// An id may also be written in the head, CSS-style — but with a space, as
/// `tag!(div #main)`, because `div#main` is not Rust: `ident#` has been a
/// reserved prefix since edition 2021, so those tokens never reach a macro at
/// all. `id: "main"` is the ordinary way to say it and reads better than the
/// space does.
///
/// The two attribute helpers are written here as they are in a template, on any
/// attribute, through the same [`TokenList`] and [`Attrs`]:
///
/// ```
/// use damask::tag;
/// # let active = true;
/// tag!(div, class: "a b");                                // a list in one string
/// tag!(div, class: @tokens("a", active.then_some("b"))); // entries, each deciding
/// tag!(div, class: @tokens("active": active));           // a name and its condition
/// tag!(div, data: @attrs(controller: "modal", open: active));
/// ```
///
/// So `data: @attrs(open: true)` writes a bare `data-open` and `open: false`
/// writes nothing at all, which is what a boolean attribute means in HTML and what
/// Damask has always done with one. A data key is taken as written — quote it
/// to spell a hyphen, since `data-user_id` is a key Damask deliberately does
/// not rewrite. An attribute name written as an ident has its underscores
/// turned into hyphens (`aria_label`), and anything an ident cannot spell is
/// quoted (`"http-equiv"`).
///
/// A void element takes no content, and saying otherwise is a compile error
/// rather than a closing tag nobody asked for.
///
/// [`Trusted`]: crate::Trusted
/// [`TokenList`]: crate::TokenList
/// [`Attrs`]: crate::Attrs
#[macro_export]
macro_rules! tag {
    ($($tt:tt)*) => {
        $crate::__tag!(($crate) $($tt)*)
    };
}

/// A sink that accumulates rendered output and owns the escaping policy.
///
/// This trait is **object-safe on purpose**. The derive emits a
/// [`Render::render_into`] that writes into `&mut dyn Renderer`, so a single
/// compiled component can be driven by any renderer — a built-in
/// ([`HtmlRenderer`] and friends) or a third-party one with custom escaping, a
/// non-`String` backing store exposed through [`finish`](Renderer::finish), or
/// streaming behavior.
///
/// # Why `Send`
///
/// An [`AsyncRender`] holds `&mut dyn Renderer` across its `.await`s, and a
/// future holding one is `Send` only if the renderer is. A server that drives
/// a render on a work-stealing executor needs that future to be `Send` — so
/// the alternative to this bound is that no async template can be rendered
/// from a request handler at all. A renderer is a buffer, which is a thing
/// that is `Send` unless it was built out of something deliberately not.
pub trait Renderer: Send {
    /// Append text with no transformation.
    ///
    /// Tags and already-safe content go through here. A tag's bytes are never
    /// laid out: the only newline one can contain is inside an attribute value,
    /// and a value is content — re-indenting a multi-line `title` would change
    /// what it says.
    fn write_raw(&mut self, s: &str);

    /// Append literal text from between a template's tags.
    ///
    /// Separate from [`write_raw`](Renderer::write_raw) because this is the
    /// only markup a renderer may lay out: it is the whitespace *between*
    /// elements, which HTML renders as a single space however much of it there
    /// is. The default treats it as raw, so a renderer that does not format
    /// needs nothing.
    fn write_text(&mut self, s: &str) {
        self.write_raw(s);
    }

    /// Append a value, applying this renderer's escaping policy.
    ///
    /// Backs the `{ … }` tag.
    fn write_escaped(&mut self, value: &dyn Display);

    /// Append a value with no escaping.
    ///
    /// Backs the `{@html … }` tag. The default formats through
    /// [`write_raw`](Renderer::write_raw); renderers backed by a buffer should
    /// override it to write in place.
    fn write_display_raw(&mut self, value: &dyn Display) {
        self.write_raw(&value.to_string());
    }

    /// Enter `levels` of nesting, for renderers that lay their output out.
    ///
    /// Indentation is a property of the *call site*, not of the component: one
    /// compiled `render_into` serves every place a component is used, and those
    /// sit at different depths. So a depth cannot be baked into a component's
    /// literals — the caller, which does know its own depth statically, opens
    /// the levels here and the renderer carries the running total.
    ///
    /// A no-op by default, which is what keeps this trait object-safe and every
    /// renderer written before it existed correct without being touched.
    fn push_indent(&mut self, levels: usize) {
        let _ = levels;
    }

    /// Leave `levels` opened by [`push_indent`](Renderer::push_indent).
    fn pop_indent(&mut self, levels: usize) {
        let _ = levels;
    }

    /// Set the indentation already written for the current line to `depth`
    /// levels below the running total, because what comes next is the end tag
    /// of an element at that depth.
    ///
    /// Only the run-time side can get this right. The last thing an element
    /// writes before its end tag may come from a `{#if}` that rendered nothing,
    /// in which case the separator standing before the tag is the one written
    /// for a *child* and is a level too deep — and whether that happened is not
    /// known until the branch is taken. So the depth is corrected here rather
    /// than baked in.
    ///
    /// Does nothing where the line is not open: an element whose content is on
    /// one line (`<span>Wi-Fi</span>`) has no separator to correct, and adding
    /// one would be the one edit that changes the document.
    fn close_line(&mut self, depth: usize) {
        let _ = depth;
    }

    /// Enter or leave a region where whitespace is significant — the content of
    /// `<pre>`, `<textarea>`, `<script>` and `<style>`, where a space this
    /// renderer added is a space the reader gets.
    ///
    /// Nests, because such an element can contain a component containing more
    /// of them.
    fn set_verbatim(&mut self, on: bool) {
        let _ = on;
    }

    /// Consume the renderer and produce the finished output.
    fn finish(self: Box<Self>) -> String;
}

/// The name of the slot `<slot/>` marks — the one with no `name="…"`, filled by
/// a caller's content that carries no `slot="…"`.
///
/// Slot names are ordinary strings and the default slot's is empty, so
/// `<slot name="…"/>` can never collide with it.
pub const DEFAULT_SLOT: &str = "";

/// One named piece of caller-supplied content, as passed to
/// [`Render::render_slots`].
///
/// The content is `Sync` as well as [`Render`], because an [`AsyncRender`]
/// holds the whole [`Slots`] it was handed across each of its `.await`s: a
/// fill that could not be shared between threads would make that future
/// non-`Send`, and a non-`Send` render cannot be awaited in a request handler.
/// The bound is here rather than on `Render` itself so that a component which
/// neither fills a slot nor awaits anything — a generic one especially — is
/// unaffected.
pub struct Slot<'a> {
    name: &'a str,
    content: Fill<'a>,
}

impl<'a> Slot<'a> {
    /// Fill the slot called `name` — [`DEFAULT_SLOT`] for `<slot/>` — with
    /// `content`.
    pub const fn new(name: &'a str, content: &'a (dyn Render + Sync)) -> Self {
        Slot {
            name,
            content: Fill::Ready(content),
        }
    }

    /// Fill it with content that has to `.await` something to render.
    ///
    /// What a template lowers `<Card><Slow/></Card>` to when the markup between
    /// the tags suspends. The callee decides nothing differently: it renders its
    /// `<slot/>` the way it always did, and only the async path — the one an
    /// awaiting template is already on — can produce the markup.
    pub const fn new_async(name: &'a str, content: &'a (dyn AsyncRender + Sync)) -> Self {
        Slot {
            name,
            content: Fill::Awaiting(content),
        }
    }
}

/// One slot's content, and whether producing it suspends.
///
/// Two shapes rather than one because there is no way to run a future to
/// completion from inside a synchronous render: an awaiting fill can only be
/// written by a caller that is itself awaiting. Which of the two a fill is, is
/// decided where the markup was written and is not something the component
/// consuming it has to know — `<slot/>` lowers to
/// [`Slots::render`](Slots::render) in a synchronous template and to
/// [`render_async`](Slots::render_async) in an awaiting one, and the second
/// handles both.
///
/// # Rendering one directly
///
/// [`Render`] is implemented, so `{@render slots.get("footer")}` still works —
/// and **panics on an awaiting fill**, naming the fix, because there is nothing
/// else it could honestly do. Use `<slot name="footer"/>` instead, which is the
/// form that has an async path. The panic is unreachable from a synchronous
/// template: a fill that awaits makes its whole enclosing template await.
#[derive(Clone, Copy)]
pub enum Fill<'a> {
    /// Markup that is already there.
    Ready(&'a (dyn Render + Sync)),
    /// Markup that has to suspend to be produced.
    Awaiting(&'a (dyn AsyncRender + Sync)),
}

impl<'a> Fill<'a> {
    /// Write this fill into `r`, suspending if it has to.
    ///
    /// Not [`AsyncRender`]: that trait has a blanket impl over every [`Render`],
    /// which this type is — so an impl here would either conflict with it or,
    /// worse, resolve to it and take the panicking synchronous path for exactly
    /// the fills that needed the other one.
    pub fn render_async<'r>(self, r: &'r mut dyn Renderer) -> RenderFuture<'r>
    where
        'a: 'r,
    {
        match self {
            Fill::Ready(content) => {
                content.render_into(r);
                Box::pin(async {})
            }
            Fill::Awaiting(content) => content.render_into_async(r),
        }
    }
}

impl Render for Fill<'_> {
    fn render_into(&self, r: &mut dyn Renderer) {
        match self {
            Fill::Ready(content) => content.render_into(r),
            Fill::Awaiting(_) => panic!(
                "this slot was filled with markup that `.await`s, and it is being rendered \
                 synchronously. Render it with `<slot/>` from a template that awaits, or with \
                 `Slots::render_async`."
            ),
        }
    }
}

/// The slot content a caller passes to one component render.
///
/// Slots are *not* props: they are content the caller supplies as markup in the
/// template — `slot="x"` on a child, or nothing for the default slot — so they
/// travel as an argument to
/// [`render_slots`](Render::render_slots) rather than as struct fields. That
/// keeps a component's struct free of `Render` type parameters however many
/// slots its template has, and lets a template add or drop a `<slot>` without
/// changing the struct.
///
/// The trade is that a slot is matched by name at render time: filling a slot a
/// template does not declare renders nothing, and a declared slot left unfilled
/// renders its fallback content.
///
/// `Slots` borrows its entries, so the fills stay on the caller's stack and can
/// borrow the caller's data with no allocation.
#[derive(Clone, Copy, Default)]
pub struct Slots<'a> {
    entries: &'a [Slot<'a>],
}

impl<'a> Slots<'a> {
    /// No slots filled — what [`Render::render_into`] passes.
    pub const EMPTY: Slots<'static> = Slots { entries: &[] };

    /// Collect fills. A name repeated in `entries` resolves to the first.
    pub const fn new(entries: &'a [Slot<'a>]) -> Self {
        Slots { entries }
    }

    /// The content filling `name`, if the caller supplied it.
    pub fn get(&self, name: &str) -> Option<Fill<'a>> {
        self.entries
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.content)
    }

    /// Whether the caller filled `name`.
    ///
    /// This is what lets a template render a wrapper only when there is
    /// something to wrap — the check a `<slot>`'s fallback cannot express,
    /// because a fallback stands in for the content, not for the markup around
    /// it.
    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// The content filling the default slot — [`get`](Slots::get) with
    /// [`DEFAULT_SLOT`], spelled so a call site need not name the empty string.
    pub fn get_default(&self) -> Option<Fill<'a>> {
        self.get(DEFAULT_SLOT)
    }

    /// Whether the caller filled the default slot.
    pub fn has_default(&self) -> bool {
        self.has(DEFAULT_SLOT)
    }

    /// Render the content filling `name`, falling back to `fallback` — the
    /// `<slot>`'s own body — when the caller left it unfilled.
    ///
    /// `indent` is the slot's depth in the template that declares it. It applies
    /// to a *fill* only: that markup was written in the caller and laid out from
    /// the caller's root, so this is what places it. The fallback is the
    /// declaring template's own markup and already carries the depth, which is
    /// why the two branches cannot share one bracket.
    pub fn render(
        &self,
        name: &str,
        r: &mut dyn Renderer,
        indent: usize,
        fallback: impl FnOnce(&mut dyn Renderer),
    ) {
        match self.get(name) {
            Some(content) => {
                r.push_indent(indent);
                content.render_into(r);
                r.pop_indent(indent);
            }
            None => fallback(r),
        }
    }

    /// Like [`render`](Slots::render), for a `<slot>` that lives in an async
    /// template — see [`AsyncRender`] for why a fallback here hands back a
    /// boxed future rather than running inline.
    pub fn render_async<'r>(
        &self,
        name: &str,
        r: &'r mut dyn Renderer,
        indent: usize,
        fallback: impl FnOnce(&'r mut dyn Renderer) -> RenderFuture<'r>,
    ) -> RenderFuture<'r>
    where
        'a: 'r,
    {
        match self.get(name) {
            Some(content) => Box::pin(async move {
                r.push_indent(indent);
                content.render_async(r).await;
                r.pop_indent(indent);
            }),
            None => fallback(r),
        }
    }
}

/// Renderable content: given a renderer, write yourself into it.
///
/// This is the shared abstraction behind composition and children/slots. Every
/// [`Component`] is `Render`; so is a [`Fragment`] built from a closure. The
/// `{@render … }` tag renders anything `Render`, so a component embeds a child
/// component or a fragment uniformly — and the child writes through the
/// *parent's* renderer, so escaping stays correct.
///
/// Object-safe, so `Box<dyn Render>` works for heterogeneous children.
pub trait Render {
    /// Write this content into `r`, with no slots filled.
    fn render_into(&self, r: &mut dyn Renderer);

    /// Write this content into `r`, resolving its `<slot>`s against `slots`.
    ///
    /// The derive overrides this with the lowered template and redirects
    /// [`render_into`](Render::render_into) here with [`Slots::EMPTY`]. The
    /// default suits content that has no slots of its own — a [`Fragment`], a
    /// hand-written `Render` — and lets such an impl stay a single method.
    fn render_slots(&self, r: &mut dyn Renderer, _slots: Slots<'_>) {
        self.render_into(r);
    }
}

impl<T: Render + ?Sized> Render for Box<T> {
    fn render_into(&self, r: &mut dyn Renderer) {
        (**self).render_into(r);
    }

    fn render_slots(&self, r: &mut dyn Renderer, slots: Slots<'_>) {
        (**self).render_slots(r, slots);
    }
}

/// A reference renders what it points at — which is what makes the borrowed
/// `&dyn Render` a [`Slots`] hands back renderable by `{@render …}`.
impl<T: Render + ?Sized> Render for &T {
    fn render_into(&self, r: &mut dyn Renderer) {
        (**self).render_into(r);
    }

    fn render_slots(&self, r: &mut dyn Renderer, slots: Slots<'_>) {
        (**self).render_slots(r, slots);
    }
}

/// `None` renders nothing — the same rule [`Attr`] follows, so absent content
/// declines to appear rather than appearing empty.
///
/// This is what lets `{@render slots.get("footer")}` stand on its own: the
/// unfilled case needs no branch around it.
impl<T: Render> Render for Option<T> {
    fn render_into(&self, r: &mut dyn Renderer) {
        if let Some(content) = self {
            content.render_into(r);
        }
    }

    fn render_slots(&self, r: &mut dyn Renderer, slots: Slots<'_>) {
        if let Some(content) = self {
            content.render_slots(r, slots);
        }
    }
}

/// A future returned by an [`AsyncRender`] method.
///
/// Boxed rather than a plain `impl Future`, because `AsyncRender` has to stay
/// object-safe the same way [`Renderer`] and [`Render`] are.
///
/// `Send`, because the executors that drive a render steal work between
/// threads: a future that is not `Send` cannot be awaited inside a request
/// handler at all. That is what [`Renderer`]'s `Send` and [`Render`]'s `Sync`
/// are there to make attainable — everything this future holds across an
/// `.await` is a renderer, a slot fill, or the component itself.
pub type RenderFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;

/// The async counterpart of [`Render`], for content whose template genuinely
/// `.await`s something.
///
/// `async fn` in a trait is not `dyn`-safe — the compiler cannot size a call
/// through `&dyn AsyncRender` without knowing the concrete future — so, like
/// the `async-trait` crate, these methods hand back an explicitly boxed
/// future instead. That is one allocation per render, which is exactly the
/// cost [`Render`] stays free of: the derive only emits `AsyncRender` for a
/// `.dmk` that contains `.await`; everything else keeps rendering through the
/// plain, allocation-free `Render`.
///
/// Every [`Render`] implementor gets `AsyncRender` for free, through the
/// blanket impl below — its future has nothing to poll but a sync call that
/// already ran to completion. That is what lets an async template embed a
/// sync child exactly as cheaply as a sync template can; the reverse does not
/// hold, since a genuinely async component has no sensible `Render` to fall
/// back to (running it would mean blocking on a future inside whatever
/// executor is already driving the caller).
pub trait AsyncRender: Sync {
    /// Write this content into `r`, with no slots filled.
    fn render_into_async<'life0, 'life1, 'async_trait>(
        &'life0 self,
        r: &'life1 mut dyn Renderer,
    ) -> RenderFuture<'async_trait>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait;

    /// Write this content into `r`, resolving its `<slot>`s against `slots`.
    ///
    /// The derive overrides this with the lowered template and redirects
    /// [`render_into_async`](AsyncRender::render_into_async) here with
    /// [`Slots::EMPTY`], mirroring [`Render::render_slots`].
    fn render_slots_async<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        r: &'life1 mut dyn Renderer,
        _slots: Slots<'life2>,
    ) -> RenderFuture<'async_trait>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        self.render_into_async(r)
    }
}

impl<T: Render + Sync + ?Sized> AsyncRender for T {
    fn render_into_async<'life0, 'life1, 'async_trait>(
        &'life0 self,
        r: &'life1 mut dyn Renderer,
    ) -> RenderFuture<'async_trait>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Render::render_into(self, r);
        Box::pin(async {})
    }

    fn render_slots_async<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 self,
        r: &'life1 mut dyn Renderer,
        slots: Slots<'life2>,
    ) -> RenderFuture<'async_trait>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        Render::render_slots(self, r, slots);
        Box::pin(async {})
    }
}

/// Wraps a `Fn(&mut dyn Renderer)` closure as [`Render`].
///
/// A blanket `impl<F: Fn(..)> Render for F` would conflict (under coherence)
/// with the per-component `impl Render`, so closures become renderable through
/// this explicit wrapper. Build one with [`fragment`].
pub struct Fragment<F>(pub F);

impl<F: Fn(&mut dyn Renderer)> Render for Fragment<F> {
    fn render_into(&self, r: &mut dyn Renderer) {
        (self.0)(r);
    }
}

/// Turn a `|r: &mut dyn Renderer| { … }` closure into renderable content.
///
/// This is what a template fragment desugars to, and how you pass ad-hoc
/// children from Rust:
///
/// ```
/// use damask::{fragment, Render, Renderer};
/// let kids = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
/// let mut buf: Box<dyn Renderer> = Box::new(damask::renderers::HtmlRenderer::new());
/// kids.render_into(buf.as_mut());
/// assert_eq!(buf.finish(), "<p>hi</p>");
/// ```
pub fn fragment<F: Fn(&mut dyn Renderer)>(f: F) -> Fragment<F> {
    Fragment(f)
}

/// Wraps a closure returning a boxed future as [`AsyncRender`] — the async
/// counterpart of [`Fragment`], for a `{#snippet}` whose enclosing template
/// awaits something. Build one with [`fragment_async`].
pub struct AsyncFragment<F>(pub F);

impl<F> AsyncRender for AsyncFragment<F>
where
    F: for<'r> Fn(&'r mut dyn Renderer) -> RenderFuture<'r> + Sync,
{
    fn render_into_async<'life0, 'life1, 'async_trait>(
        &'life0 self,
        r: &'life1 mut dyn Renderer,
    ) -> RenderFuture<'async_trait>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        (self.0)(r)
    }
}

/// Turn a `|r: &mut dyn Renderer| { Box::pin(async move { … }) }` closure into
/// async-renderable content — the async counterpart of [`fragment`].
pub fn fragment_async<F>(f: F) -> AsyncFragment<F>
where
    F: for<'r> Fn(&'r mut dyn Renderer) -> RenderFuture<'r>,
{
    AsyncFragment(f)
}

/// Widen a reference to `&dyn Display` for [`Renderer::write_escaped`] and
/// [`Renderer::write_display_raw`].
///
/// Generated code routes every `{ … }` and `{@html … }` value through this
/// instead of unsizing at the call site. The two are equivalent to rustc, but
/// passing `&(expr)` straight to a `&dyn Display` parameter unsizes whatever
/// type inference has arrived at so far — and when `expr` is a snippet
/// parameter, whose type is still an inference variable, rust-analyzer resolves
/// that variable to `dyn Display` itself and then reports every argument at the
/// call site as a mismatch. Going through a generic function makes the type a
/// plain `T: Display` bound, so the parameter is inferred from the call site as
/// usual and the coercion happens where `T` is already known.
///
/// `T` is `Sized`, which unsizing requires. An already-unsized value therefore
/// needs a reference of its own: write `{&*boxed}` rather than `{*boxed}`.
#[inline]
pub fn as_display<'a, T: Display + 'a>(value: &'a T) -> &'a dyn Display {
    value
}

/// A renderable component.
///
/// Generated by the [`Component`](macro@Component) derive from a struct plus its
/// `.dmk` template. `Component: Render` adds a default renderer and the
/// convenience [`render`](Component::render). Object-safe, so `&dyn Component`
/// and `Vec<Box<dyn Component>>` work.
pub trait Component: Render {
    /// The renderer [`render`](Component::render) uses when the caller names
    /// none.
    ///
    /// The derive implements this as a boxed [`HtmlRenderer`], for every
    /// component: `.dmk` templates are HTML, and that is the only host language
    /// there is. It is a trait method rather than a constant so that a
    /// hand-written `Component` can answer differently, and so that adding a
    /// second host language later is a change to the derive rather than to this
    /// trait's shape.
    ///
    /// It is not how you use a custom renderer — for that, call
    /// [`Render::render_into`] with the renderer you want, which is what makes
    /// one compiled component work with any of them.
    fn default_renderer(&self) -> Box<dyn Renderer>;

    /// Render to a `String` using the [default renderer](Component::default_renderer).
    fn render(&self) -> String {
        self.render_with(Slots::EMPTY)
    }

    /// Like [`render`](Component::render), but fills the template's `<slot>`s —
    /// the Rust-side equivalent of `<Comp>…</Comp>` in a template.
    ///
    /// ```
    /// use damask::{fragment, Slot, Slots};
    /// # use damask::{Component, Render, Renderer};
    /// # struct Layout;
    /// # impl Render for Layout {
    /// #     fn render_into(&self, r: &mut dyn Renderer) { self.render_slots(r, Slots::EMPTY) }
    /// #     fn render_slots(&self, r: &mut dyn Renderer, slots: Slots<'_>) {
    /// #         r.write_raw("<main>");
    /// #         slots.render(damask::DEFAULT_SLOT, r, 0, |_| {});
    /// #         r.write_raw("</main>");
    /// #     }
    /// # }
    /// # impl Component for Layout {
    /// #     fn default_renderer(&self) -> Box<dyn Renderer> { Box::new(damask::HtmlRenderer::new()) }
    /// # }
    /// let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>hi</p>"));
    /// let out = Layout.render_with(Slots::new(&[Slot::new(damask::DEFAULT_SLOT, &body)]));
    /// assert_eq!(out, "<main><p>hi</p></main>");
    /// ```
    fn render_with(&self, slots: Slots<'_>) -> String {
        let mut r = self.default_renderer();
        self.render_slots(r.as_mut(), slots);
        r.finish()
    }
}

/// The async counterpart of [`Component`], for a component whose template
/// genuinely `.await`s something — see [`AsyncRender`] for why that means a
/// separate trait rather than one method that is sometimes async.
///
/// Every [`Component`] gets `AsyncComponent` for free (via [`AsyncRender`]'s
/// blanket impl), so `render_async`/`render_with_async` work uniformly across
/// sync and async components; only a genuinely async component's derive
/// implements this trait directly, since such a component has no sync
/// `Component` to fall back to.
pub trait AsyncComponent: AsyncRender {
    /// The renderer [`render_async`](AsyncComponent::render_async) uses when
    /// the caller names none. See [`Component::default_renderer`].
    fn default_renderer(&self) -> Box<dyn Renderer>;

    /// Render to a `String` using the [default renderer](AsyncComponent::default_renderer).
    fn render_async<'life0, 'async_trait>(
        &'life0 self,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        self.render_with_async(Slots::EMPTY)
    }

    /// Like [`render_async`](AsyncComponent::render_async), but fills the
    /// template's `<slot>`s — see [`Component::render_with`].
    fn render_with_async<'life0, 'life1, 'async_trait>(
        &'life0 self,
        slots: Slots<'life1>,
    ) -> Pin<Box<dyn Future<Output = String> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            let mut r = self.default_renderer();
            self.render_slots_async(r.as_mut(), slots).await;
            r.finish()
        })
    }
}

impl<T: Component + Sync + ?Sized> AsyncComponent for T {
    fn default_renderer(&self) -> Box<dyn Renderer> {
        Component::default_renderer(self)
    }
}

/// Common imports for authoring and using components.
///
/// `Component` here is both the trait and its derive macro.
pub mod prelude {
    pub use crate::attr::{
        Attr, AttrSet, AttrSpread, AttrValue, Attrs, IntoAttrValue, TokenItem, TokenList,
    };
    pub use crate::renderers::{HtmlRenderer, StringRenderer, Whitespace};
    pub use crate::{
        AsyncComponent, AsyncRender, Component, DEFAULT_SLOT, Render, RenderFuture, Renderer, Slot,
        Slots, fragment, fragment_async,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderers::{HtmlRenderer, StringRenderer, escape_html};

    // A hand-written component, standing in for macro output.
    struct Greeting {
        name: String,
    }

    impl Render for Greeting {
        fn render_into(&self, r: &mut dyn Renderer) {
            r.write_raw("Hello ");
            r.write_escaped(&self.name);
            r.write_raw("!");
        }
    }

    impl Component for Greeting {
        fn default_renderer(&self) -> Box<dyn Renderer> {
            Box::new(HtmlRenderer::new())
        }
    }

    #[test]
    fn render_uses_default_renderer_and_escapes() {
        let g = Greeting {
            name: "<Ada>".into(),
        };
        assert_eq!(g.render(), "Hello &lt;Ada&gt;!");
    }

    #[test]
    fn render_into_accepts_any_renderer() {
        // Drive the same component through a bespoke renderer: prove the seam.
        let g = Greeting { name: "Bob".into() };
        let mut custom: Box<dyn Renderer> = Box::new(StringRenderer::with_escape(escape_html));
        g.render_into(custom.as_mut());
        assert_eq!(custom.finish(), "Hello Bob!");
    }

    #[test]
    fn components_are_object_safe() {
        let items: Vec<Box<dyn Component>> = vec![
            Box::new(Greeting { name: "a".into() }),
            Box::new(Greeting { name: "b".into() }),
        ];
        let out: String = items.iter().map(|c| c.render()).collect();
        assert_eq!(out, "Hello a!Hello b!");
    }

    fn rendered(content: impl Render) -> String {
        let mut r: Box<dyn Renderer> = Box::new(HtmlRenderer::new());
        content.render_into(r.as_mut());
        r.finish()
    }

    /// What `{@render slots.get(…)}` leans on: the borrowed content a `Slots`
    /// hands back is renderable, and an unfilled slot renders nothing rather
    /// than forcing a branch at every use.
    #[test]
    fn a_slot_lookup_is_renderable_either_way() {
        let g = Greeting { name: "Ada".into() };
        let entries = [Slot::new("body", &g)];
        let slots = Slots::new(&entries);
        assert_eq!(rendered(slots.get("body")), "Hello Ada!");
        assert_eq!(rendered(slots.get("absent")), "");
    }

    #[test]
    fn slots_report_what_the_caller_filled() {
        let g = Greeting { name: "Ada".into() };
        let entries = [Slot::new(DEFAULT_SLOT, &g)];
        let slots = Slots::new(&entries);
        assert!(slots.has_default() && slots.has(DEFAULT_SLOT));
        assert!(!slots.has("body"));
        assert_eq!(rendered(slots.get_default()), "Hello Ada!");
        assert!(!Slots::EMPTY.has_default());
    }

    /// A single-threaded, no-IO executor for these tests: no runtime
    /// dependency, and every future here resolves without ever really
    /// suspending, so busy-polling with a no-op waker is enough.
    fn block_on<F: Future>(f: F) -> F::Output {
        let mut f = std::pin::pin!(f);
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        loop {
            if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
                return v;
            }
        }
    }

    /// A hand-written component that genuinely awaits something, standing in
    /// for what the derive emits for a `.dmk` containing `.await`.
    struct AsyncGreeting {
        name: String,
    }

    impl AsyncRender for AsyncGreeting {
        fn render_into_async<'life0, 'life1, 'async_trait>(
            &'life0 self,
            r: &'life1 mut dyn Renderer,
        ) -> RenderFuture<'async_trait>
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async move {
                let name = std::future::ready(self.name.clone()).await;
                r.write_raw("Hello ");
                r.write_escaped(&name);
                r.write_raw("!");
            })
        }
    }

    impl AsyncComponent for AsyncGreeting {
        fn default_renderer(&self) -> Box<dyn Renderer> {
            Box::new(HtmlRenderer::new())
        }
    }

    #[test]
    fn a_genuinely_async_component_renders_through_render_async() {
        let g = AsyncGreeting {
            name: "<Ada>".into(),
        };
        assert_eq!(block_on(g.render_async()), "Hello &lt;Ada&gt;!");
    }

    /// The blanket `AsyncRender` a plain `Render` type gets for free — no
    /// `.dmk` awaits anything, so an async parent can still embed it.
    #[test]
    fn a_sync_component_renders_through_the_async_path_for_free() {
        let g = Greeting { name: "Bob".into() };
        let mut r: Box<dyn Renderer> = Box::new(HtmlRenderer::new());
        block_on(AsyncRender::render_into_async(&g, r.as_mut()));
        assert_eq!(r.finish(), "Hello Bob!");
        assert_eq!(block_on(g.render_async()), "Hello Bob!");
    }

    #[test]
    fn slots_render_async_fills_and_falls_back() {
        let g = Greeting { name: "Ada".into() };
        let entries = [Slot::new("body", &g)];
        let slots = Slots::new(&entries);

        let filled = block_on(async {
            let mut r: Box<dyn Renderer> = Box::new(HtmlRenderer::new());
            slots
                .render_async("body", r.as_mut(), 0, |r| {
                    Box::pin(async { r.write_raw("fallback") })
                })
                .await;
            r.finish()
        });
        assert_eq!(filled, "Hello Ada!");

        let fallback = block_on(async {
            let mut r: Box<dyn Renderer> = Box::new(HtmlRenderer::new());
            slots
                .render_async("absent", r.as_mut(), 0, |r| {
                    Box::pin(async { r.write_raw("fallback") })
                })
                .await;
            r.finish()
        });
        assert_eq!(fallback, "fallback");
    }

    /// The capability the whole `Fill` split exists for: a wrapper's slot may
    /// be filled with markup that suspends, so an awaiting component can be the
    /// child of one that is not.
    #[test]
    fn a_slot_may_be_filled_with_content_that_awaits() {
        let slow = AsyncGreeting { name: "Ada".into() };
        let entries = [Slot::new_async(DEFAULT_SLOT, &slow)];
        let slots = Slots::new(&entries);

        let out = block_on(async {
            let mut r: Box<dyn Renderer> = Box::new(HtmlRenderer::new());
            slots
                .render_async(DEFAULT_SLOT, r.as_mut(), 0, |r| {
                    Box::pin(async { r.write_raw("fallback") })
                })
                .await;
            r.finish()
        });

        assert_eq!(out, "Hello Ada!");
    }

    /// An awaiting fill is a fill: everything that asks *whether* a slot was
    /// filled has to say yes, or a wrapper drawn only around real content
    /// would skip exactly the content that was expensive to produce.
    #[test]
    fn an_awaiting_fill_counts_as_filled() {
        let slow = AsyncGreeting { name: "Ada".into() };
        let entries = [Slot::new_async(DEFAULT_SLOT, &slow)];
        let slots = Slots::new(&entries);

        assert!(slots.has_default());
        assert!(slots.get_default().is_some());
    }

    /// There is nothing else it could do: a future cannot be run to completion
    /// from inside a synchronous render. Unreachable from a template — a fill
    /// that awaits makes its whole enclosing template await — so what this pins
    /// is that a hand-built `Slots` says so rather than rendering nothing.
    #[test]
    #[should_panic(expected = "Slots::render_async")]
    fn rendering_an_awaiting_fill_synchronously_says_so() {
        let slow = AsyncGreeting { name: "Ada".into() };
        let entries = [Slot::new_async(DEFAULT_SLOT, &slow)];
        let slots = Slots::new(&entries);

        let _ = rendered(slots.get_default());
    }
}
