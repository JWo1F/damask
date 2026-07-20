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
//! to [`Render::render_slots`]. See [`Slots`] for what that buys and costs.
//!
//! A prop must be passed unless its type says what leaving it out means:
//! `Option<_>` is `None`. `#[component(default)]` on the struct extends that to
//! every prop, filling the skipped ones from its `Default`. See [`props`] for
//! how a call site — which cannot see the struct it is building — is held to
//! that.

use std::fmt::Display;

pub mod attr;
pub mod props;
pub mod renderers;

pub use attr::{Attr, AttrSpread, ClassItem, ClassList};
pub use renderers::{HtmlRenderer, Whitespace};

/// Derive macro that generates a [`Component`] impl from a struct's paired
/// `.dmk` template. Shares its name with the trait (like `serde::Serialize`), so
/// `use damask::Component;` brings both into scope.
pub use damask_macros::Component;

/// A sink that accumulates rendered output and owns the escaping policy.
///
/// This trait is **object-safe on purpose**. The derive emits a
/// [`Render::render_into`] that writes into `&mut dyn Renderer`, so a single
/// compiled component can be driven by any renderer — a built-in
/// ([`HtmlRenderer`] and friends) or a third-party one with custom escaping, a
/// non-`String` backing store exposed through [`finish`](Renderer::finish), or
/// streaming behavior.
pub trait Renderer {
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

/// The name of the slot `<slot/>` fills — the one with no `name="…"`.
///
/// Slot names are ordinary strings and the default slot's is empty, so
/// `<slot name="…"/>` can never collide with it.
pub const DEFAULT_SLOT: &str = "";

/// One named piece of caller-supplied content, as passed to
/// [`Render::render_slots`].
pub struct Slot<'a> {
    name: &'a str,
    content: &'a dyn Render,
}

impl<'a> Slot<'a> {
    /// Fill the slot called `name` — [`DEFAULT_SLOT`] for `<slot/>` — with
    /// `content`.
    pub const fn new(name: &'a str, content: &'a dyn Render) -> Self {
        Slot { name, content }
    }
}

/// The slot content a caller passes to one component render.
///
/// Slots are *not* props: they are content the caller supplies positionally in
/// the template, so they travel as an argument to
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
    pub fn get(&self, name: &str) -> Option<&'a dyn Render> {
        self.entries
            .iter()
            .find(|s| s.name == name)
            .map(|s| s.content)
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
/// `.dmk` template. `Component: Render` adds a default renderer (chosen by the
/// template's host language) and the convenience [`render`](Component::render).
/// Object-safe, so `&dyn Component` and `Vec<Box<dyn Component>>` work.
pub trait Component: Render {
    /// The renderer chosen by the template's host language, used by
    /// [`render`](Component::render).
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

/// Common imports for authoring and using components.
///
/// `Component` here is both the trait and its derive macro.
pub mod prelude {
    pub use crate::attr::{Attr, AttrSpread, ClassItem, ClassList};
    pub use crate::renderers::{HtmlRenderer, StringRenderer, Whitespace};
    pub use crate::{Component, DEFAULT_SLOT, Render, Renderer, Slot, Slots, fragment};
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
}
