//! Lower a parsed [`Template`] to the body of a `render_into` method, as a
//! string of Rust source.
//!
//! This is the single source of truth for template → Rust code generation,
//! shared by two consumers so they can never disagree about what a template
//! means:
//!
//! - the `Component` derive in `damask-macros`, which splices this body into an
//!   `impl` at compile time, and
//! - the language server, which wraps the same body in a virtual `impl` block
//!   and hands it to `rust-analyzer`.
//!
//! It emits *only* the block body (`{ … }`); the caller supplies the
//! surrounding `fn render_into(&self, __damask: &mut dyn Renderer)` signature.
//!
//! Alongside the text, [`lower_mapped`] returns a [`SourceMap`]: for every Rust
//! fragment copied verbatim from the template, a correspondence between its
//! `.dmk` byte range and the byte range it occupies in the generated body.
//! Because each such copy is byte-identical, the two ranges are always the same
//! length, so the language server can translate a position from one side to the
//! other by a constant offset.

use crate::{
    Attr, AttrPart, AttrValue, ClassTerm, DataTerm, Element, ElementKind, ForNode, IfNode, Node,
    SnippetNode, Span, Spanned, Template, is_void_element,
};

/// A verbatim correspondence between a `.dmk` source range and the generated
/// Rust range it was copied into. Both ranges cover byte-identical text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mapping {
    /// Byte range in the `.dmk` template source.
    pub source: Span,
    /// Byte range in the body string returned by [`lower_mapped`].
    pub generated: Span,
}

/// The ordered set of source↔generated correspondences produced by lowering.
/// Entries are pushed in generated order, which is also source order: a
/// `{#for pat in expr}` header emits `pat` before `expr`, exactly as written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceMap {
    pub mappings: Vec<Mapping>,
}

/// Accumulates generated Rust plus the source map as fragments are emitted.
struct Emit {
    out: String,
    map: SourceMap,
    /// Whether the last literal emitted ended with a newline run, so the next
    /// one must not start with a second. Two runs in a row are what a `{# … #}`
    /// comment leaves when it vanishes — the text before it and the text after
    /// it are separate nodes, each ending and beginning with the same
    /// separator — and what a `{#if}` leaves at each edge of its body.
    ///
    /// Dropping the duplicate is safe for the same reason resizing one is: it
    /// is never the *last* run between two things, because the run that made
    /// this flag true is still there.
    at_line_start: bool,
    /// The path scaffolding reaches this crate through, substituted for
    /// [`PLACEHOLDER`] as each piece is emitted.
    krate: String,
    /// Whether this template contains `.await` anywhere (see
    /// [`crate::needs_async`]), decided once before lowering starts. Every
    /// render call this template makes — a nested `<Component/>`, a
    /// `{@render …}`, a `<slot>`'s fallback — goes through the async form and
    /// is awaited when this is set, and through the plain sync form when it
    /// is not. A template does not mix the two: the whole body sits inside
    /// one `fn` (sync) or one `async move { … }` (async), so every call
    /// inside it has to match.
    is_async: bool,
}

/// What the lowering writes where the crate path goes.
///
/// Scaffolding is emitted as string literals with the path embedded in them, so
/// this stands in for it and [`Emit::raw`] substitutes. Substituting beats
/// emitting one `use … as` alias at the top of the block and leaning on it: the
/// generated body is a scope a template's own Rust runs in, and a lowering that
/// binds no names there cannot collide with anything the author writes.
const PLACEHOLDER: &str = "__damask_krate";

impl Emit {
    fn new(krate: &str, is_async: bool) -> Self {
        Emit {
            out: String::new(),
            map: SourceMap::default(),
            at_line_start: false,
            krate: krate.to_string(),
            is_async,
        }
    }

    /// Append scaffolding that does not correspond to any source range.
    ///
    /// Substitution happens here rather than after the fact so that the source
    /// map, which records offsets into `out` as it grows, stays correct — and it
    /// is safe to do here because a *fragment* (the author's own Rust) goes
    /// through [`frag_sub`](Emit::frag_sub) and is never rewritten.
    fn raw(&mut self, s: &str) {
        match s.contains(PLACEHOLDER) {
            true => self.out.push_str(&s.replace(PLACEHOLDER, &self.krate)),
            false => self.out.push_str(s),
        }
    }

    /// Splice a source fragment verbatim and record its mapping.
    fn frag(&mut self, frag: &Spanned) {
        self.frag_sub(frag.span, &frag.text);
    }

    /// Splice `text` verbatim, recording that it came from `source`. `text` must
    /// be exactly the bytes `source` covers (callers pass a sub-slice of a
    /// fragment together with the matching sub-span).
    fn frag_sub(&mut self, source: Span, text: &str) {
        debug_assert_eq!(
            source.len(),
            text.len(),
            "mapped fragment must be a verbatim copy",
        );
        let start = self.out.len();
        self.out.push_str(text);
        let generated = Span::new(start, self.out.len());
        self.map.mappings.push(Mapping { source, generated });
    }
}

/// How a lowered body names this crate, when the caller does not say.
///
/// It is a parameter at all because a framework may re-export Damask rather than
/// have its users depend on it — generated code that said `::damask` outright
/// would only compile where `damask` is a *direct* dependency, since a leading
/// `::` resolves against the extern prelude and nothing else.
pub const DEFAULT_CRATE: &str = "::damask";

/// Lower a template to the body of `render_into`, as a string of Rust source.
///
/// The result is a single brace-delimited block; the caller parses it once so
/// control-flow tags whose braces span multiple `{ }` block tags balance as one
/// Rust block.
pub fn lower(template: &Template) -> Result<String, String> {
    lower_with(template, DEFAULT_CRATE)
}

/// Like [`lower`], but names this crate `krate` — see [`DEFAULT_CRATE`].
pub fn lower_with(template: &Template, krate: &str) -> Result<String, String> {
    lower_mapped_with(template, krate).map(|(body, _)| body)
}

/// Like [`lower`], but also returns the [`SourceMap`] tying generated ranges
/// back to the template.
pub fn lower_mapped(template: &Template) -> Result<(String, SourceMap), String> {
    lower_mapped_with(template, DEFAULT_CRATE)
}

/// Like [`lower_mapped`], but names this crate `krate` — see [`DEFAULT_CRATE`].
pub fn lower_mapped_with(template: &Template, krate: &str) -> Result<(String, SourceMap), String> {
    let mut e = Emit::new(krate, crate::needs_async(template));
    e.raw("{\n");
    // Bring `Component`/`Render` (and their async counterparts) into scope
    // (unnamed) so `child.render()`, `child.render_async().await`, and
    // `{@render …}`-style calls resolve without the author importing the
    // traits.
    e.raw(
        "#[allow(unused_imports)] use __damask_krate::{Component as _, Render as _, AsyncComponent as _, AsyncRender as _, props::Rest as _};\n",
    );
    // One fallback trait per attribute name written on a component here, so
    // that a name the component does not declare as a prop reaches its bag
    // instead of failing to resolve. See `crate::rest` for why the choice is
    // left to method resolution rather than made in this pass.
    for name in crate::rest::fallback_names(&template.nodes) {
        e.raw(&crate::rest::fallback_trait(&name, krate));
    }
    // And the one every attribute that can only go to the bag travels through,
    // which is there so that a component without a bag fails on the trait bound
    // — where the message lives — rather than on a missing method.
    if crate::rest::needs_any_trait(&template.nodes) {
        e.raw(&crate::rest::any_trait(krate));
    }
    // The caller's fills, under a name a template may actually write. `<slot>`
    // resolves against them implicitly; this is what lets a template *ask* —
    // whether a slot was filled, and where to put the answer. Shadowable on
    // purpose: it is an ordinary binding, so a template that wants the name for
    // something else may take it.
    e.raw("#[allow(unused_variables)] let slots = __damask_slots;\n");
    emit_nodes(&template.nodes, Layout::ROOT, &mut e)?;
    e.raw("}\n");
    Ok((e.out, e.map))
}

/// Spaces per level of nesting in the generated literals. Matches
/// `damask::renderers::INDENT_WIDTH`, which supplies the other half of the sum.
const INDENT_WIDTH: usize = 2;

/// Where a run of nodes sits, for laying its literal whitespace out.
///
/// A template is laid out from *its own* root: a component knows how deep its
/// markup is inside itself and nothing about the call site that renders it, so
/// the two depths are added at run time (see `Renderer::push_indent`). This is
/// the half that is static.
#[derive(Debug, Clone, Copy)]
struct Layout {
    /// Nesting depth of these nodes, in HTML elements.
    depth: usize,
    /// Depth the last whitespace run closes to — the enclosing element's, since
    /// what follows it is that element's end tag.
    closing: usize,
    /// Inside `<pre>` and friends, where a space is a space the reader gets.
    verbatim: bool,
}

impl Layout {
    /// The top of a template, and of any markup relocated into another one
    /// (slot fills, snippet bodies), which is laid out from its own root for
    /// the same reason a component is.
    const ROOT: Layout = Layout {
        depth: 0,
        closing: 0,
        verbatim: false,
    };

    /// The layout for the children of an element at this layout's depth.
    fn inside(self, verbatim: bool) -> Layout {
        Layout {
            depth: self.depth + 1,
            closing: self.depth,
            verbatim: self.verbatim || verbatim,
        }
    }

    /// The layout for a control-flow body — `{#if}`, `{#for}`. These are not
    /// elements and produce no tag, so they do not nest the output.
    ///
    /// `closing` resets to the body's own depth: the last node of a *body* is
    /// not the last before an end tag, it is followed by whatever comes after
    /// the `{/if}`, which is a sibling at the same depth. Where the body really
    /// is the last thing in its element, `Renderer::close_line` corrects the
    /// run at run time — which is the only place that can tell.
    fn same(self) -> Layout {
        Layout {
            closing: self.depth,
            ..self
        }
    }
}

/// HTML elements whose content is not laid out, because whitespace inside them
/// is significant (`pre`, `textarea`) or is program text whose meaning a stray
/// space can change (`script`, `style`).
fn is_verbatim_element(tag: &str) -> bool {
    matches!(tag, "pre" | "textarea" | "script" | "style")
}

/// Re-lay out the literal whitespace of a text node.
///
/// Every run of whitespace containing a newline collapses to exactly one
/// newline plus this template's own indentation — which is what removes the
/// blank lines a `{# … #}` comment or a control-flow tag leaves behind when it
/// vanishes, and what makes the indentation the tree's rather than the
/// author's.
///
/// The transform only ever *resizes* a run that already contains a newline. It
/// never introduces a newline between two things the author wrote adjacent, and
/// never removes the last newline separating two things they wrote apart. HTML
/// renders any such run as a single space wherever whitespace is insignificant,
/// so the document is unchanged — and where whitespace *is* significant, the
/// run is inside a verbatim element and is not touched at all.
///
/// Runs with no newline are the author's own spacing inside a line and are left
/// exactly as written.
fn relayout_text(s: &str, layout: Layout, is_last: bool, at_line_start: &mut bool) -> String {
    if layout.verbatim {
        *at_line_start = false;
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;

    // A leading run when the previous literal already ended in one: the
    // separator has been written, so this is the duplicate.
    if *at_line_start {
        rest = rest.trim_start_matches([' ', '\t', '\r', '\n']);
    }
    while let Some(nl) = rest.find('\n') {
        // Back up over any spaces already copied that belong to this run: the
        // run starts at the last non-whitespace byte, not at the newline.
        let head = &rest[..nl];
        let keep = head.trim_end_matches([' ', '\t', '\r', '\n']).len();
        out.push_str(&head[..keep]);

        let after = &rest[nl + 1..];
        let run = after.len() - after.trim_start_matches([' ', '\t', '\r', '\n']).len();
        rest = &after[run..];

        // The final run of the final text node in an element is followed by
        // that element's end tag, so it closes to the element's own depth.
        let depth = if rest.is_empty() && is_last {
            layout.closing
        } else {
            layout.depth
        };
        out.push('\n');
        out.extend(std::iter::repeat_n(' ', depth * INDENT_WIDTH));
    }
    out.push_str(rest);
    // Trailing whitespace with a newline is what the next literal may skip. An
    // empty result wrote nothing and so cannot have moved the line.
    if !out.is_empty() {
        *at_line_start = out
            .rsplit_once('\n')
            .is_some_and(|(_, tail)| tail.chars().all(|c| c == ' ' || c == '\t'));
    }
    out
}

fn emit_nodes(nodes: &[Node], layout: Layout, e: &mut Emit) -> Result<(), String> {
    for (i, node) in nodes.iter().enumerate() {
        emit_node(node, layout, i + 1 == nodes.len(), e)?;
    }
    Ok(())
}

fn emit_node(node: &Node, layout: Layout, is_last: bool, e: &mut Emit) -> Result<(), String> {
    match node {
        // Text becomes an escaped string literal — not a verbatim copy, so it is
        // not mapped here; text positions belong to the HTML virtual document.
        Node::Text(text) => {
            let laid_out = relayout_text(text.as_str(), layout, is_last, &mut e.at_line_start);
            if !laid_out.is_empty() {
                e.raw(&format!("__damask.write_text({laid_out:?});\n"));
            }
        }
        Node::Expr(code) => emit_expr(code, e),
        Node::Html(code) => {
            require_expr(code.as_str(), "{@html … }")?;
            e.raw("__damask.write_display_raw(__damask_krate::as_display(&(");
            e.frag(code);
            e.raw(")));\n");
            e.at_line_start = false;
        }
        // A snippet or fragment is laid out from its own root, like a
        // component, so the depth of the site rendering it is added here.
        Node::Render(code) => {
            require_expr(code.as_str(), "{@render … }")?;
            let is_async = e.is_async;
            indented(layout.depth, e, |e| {
                if is_async {
                    e.raw("__damask_krate::AsyncRender::render_into_async(&(");
                    e.frag(code);
                    e.raw("), &mut *__damask).await;\n");
                } else {
                    e.raw("__damask_krate::Render::render_into(&(");
                    e.frag(code);
                    e.raw("), &mut *__damask);\n");
                }
            });
            e.at_line_start = false;
        }
        Node::If(if_node) => emit_if(if_node, layout.same(), e)?,
        Node::For(node) => emit_for(node, layout.same(), e)?,
        Node::Snippet(snippet) => emit_snippet(snippet, e)?,
        Node::Element(element) => emit_element(element, layout, e)?,
    }
    Ok(())
}

/// Wrap `body` in the renderer calls that add `depth` levels to whatever it
/// writes. Emits nothing when there is no depth to add, so markup at the root of
/// a template costs nothing.
fn indented(depth: usize, e: &mut Emit, body: impl FnOnce(&mut Emit)) {
    if depth == 0 {
        body(e);
        return;
    }
    e.raw(&format!("__damask.push_indent({depth});\n"));
    body(e);
    e.raw(&format!("__damask.pop_indent({depth});\n"));
}

/// A `{ … }` block: splice it as a statement (no output) if it's a binding or
/// ends in `;`, otherwise print its value — escaped, unless it is
/// [`Trusted`](damask::Trusted) markup.
///
/// Which of the two it is belongs to the *value*: `damask::splice` hands it to
/// `Value`, whose `Trusted` impl writes markup through and whose blanket impl
/// escapes everything else. A generic function rather than a method call, so a
/// value whose type is not settled yet — a `{#snippet}` parameter — still
/// infers from wherever the snippet is rendered.
fn emit_expr(code: &Spanned, e: &mut Emit) {
    // The parser trims tag bodies, so `code.text` is already trimmed; the map
    // therefore aligns with the fragment exactly.
    let trimmed = code.text.trim();
    if is_statement(trimmed) {
        // A `use` may import something used only in a sibling scope; don't warn.
        if starts_with_kw(trimmed, "use") {
            e.raw("#[allow(unused_imports)] ");
        }
        e.frag(code);
        if !trimmed.ends_with(';') {
            e.raw(";");
        }
        e.raw("\n");
    } else if trimmed.contains(';') {
        // Multiple statements ending in an expression need a block; the block's
        // value is a temporary, so borrowing it is fine.
        e.raw("__damask_krate::splice(&({ ");
        e.frag(code);
        e.raw(" }), &mut *__damask);\n");
    } else {
        // A plain expression: borrow it directly (no block) so field access
        // like `self.name` borrows rather than moves out of `&self`.
        e.raw("__damask_krate::splice(&(");
        e.frag(code);
        e.raw("), &mut *__damask);\n");
    }
    // A `{use}` or `{let}` writes nothing, so it cannot have moved the line —
    // and a template's header of `{use}` tags is otherwise a run of blank lines
    // at the top of every page it renders.
    if !is_statement(trimmed) {
        e.at_line_start = false;
    }
}

/// Whether a `{ … }` block is a statement or item (yields no value to print).
fn is_statement(trimmed: &str) -> bool {
    const ITEM_KEYWORDS: &[&str] = &[
        "let", "const", "use", "fn", "static", "type", "struct", "enum", "trait", "impl", "mod",
    ];
    trimmed.ends_with(';') || ITEM_KEYWORDS.iter().any(|kw| starts_with_kw(trimmed, kw))
}

fn starts_with_kw(s: &str, kw: &str) -> bool {
    s.strip_prefix(kw)
        .and_then(|r| r.chars().next())
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        && s.starts_with(kw)
}

/// Which branch runs is a run-time fact, so the line position after the whole
/// construct is only known where every path agrees on it — including the path
/// that runs no branch at all, which leaves it as it was. Each branch therefore
/// starts from the state before the tag, and the states they end in are met.
fn emit_if(if_node: &IfNode, layout: Layout, e: &mut Emit) -> Result<(), String> {
    let before = e.at_line_start;
    let mut agreed = before;
    for (i, (cond, body)) in if_node.branches.iter().enumerate() {
        e.at_line_start = before;
        require_expr(cond.as_str(), "{#if … }")?;
        if i == 0 {
            e.raw("if ");
        } else {
            e.raw("} else if ");
        }
        e.frag(cond);
        e.raw(" {\n");
        emit_nodes(body, layout, e)?;
        agreed &= e.at_line_start;
    }
    if let Some(otherwise) = &if_node.otherwise {
        e.raw("} else {\n");
        e.at_line_start = before;
        emit_nodes(otherwise, layout, e)?;
        agreed &= e.at_line_start;
    }
    e.raw("}\n");
    e.at_line_start = agreed;
    Ok(())
}

/// `{#for pat in expr}` → `for pat in expr {`. The header is Rust verbatim, so
/// enumeration and the like are written on `expr` (`xs.iter().enumerate()`);
/// the lowering has no special case of its own.
fn emit_for(node: &ForNode, layout: Layout, e: &mut Emit) -> Result<(), String> {
    // An empty iterator runs the body no times, so the state after the loop is
    // known only where the body agrees with the state before it.
    let before = e.at_line_start;
    let (pat, expr) = (node.pat.as_str().trim(), node.expr.as_str().trim());
    if pat.is_empty() || expr.is_empty() {
        return Err("malformed `{#for}`".into());
    }
    e.raw("for ");
    e.frag(&node.pat);
    e.raw(" in ");
    e.frag(&node.expr);
    e.raw(" {\n");
    emit_nodes(&node.body, layout, e)?;
    e.raw("}\n");
    e.at_line_start &= before;
    Ok(())
}

fn emit_snippet(snippet: &SnippetNode, e: &mut Emit) -> Result<(), String> {
    if snippet.name.as_str().is_empty() {
        return Err("`{#snippet}` needs a name".into());
    }
    // Whether *this* snippet's own body awaits something — independent of
    // whether the enclosing template does. A plain `Fragment` still renders
    // fine from inside an async template (any `Render` gets `AsyncRender` for
    // free), so only a snippet that genuinely needs to suspend pays for the
    // boxed-future shape.
    let snippet_awaits = crate::awaits::nodes_need_async(&snippet.body);
    if snippet_awaits && !snippet.params.as_str().trim().is_empty() {
        // A parameterized snippet's closure has to stay callable more than
        // once (it implements `Render`/`AsyncRender` by `&self`), but an
        // `async move` block claims full ownership of everything it touches
        // on *every* call — so a non-`Copy` parameter moved in once could not
        // be moved in again on a second call. There is no borrow-based way
        // around this that survives arbitrary parameter types, so it is
        // rejected instead of miscompiling into a borrow-checker error deep
        // in generated code.
        return Err(format!(
            "`{{#snippet {}(…)}}` cannot both take parameters and `.await` in its own body; \
             move the `.await` to where it's rendered instead — `{{@render {}(some_async_call().await)}}` \
             — and have the snippet body use the already-resolved value",
            snippet.name.as_str(),
            snippet.name.as_str(),
        ));
    }
    if snippet.params.as_str().is_empty() {
        e.raw("let ");
        e.frag(&snippet.name);
        if snippet_awaits {
            e.raw(" = __damask_krate::fragment_async(|__damask: &mut dyn __damask_krate::Renderer| { ::std::boxed::Box::pin(async move {\n");
        } else {
            e.raw(" = __damask_krate::fragment(|__damask: &mut dyn __damask_krate::Renderer| {\n");
        }
    } else {
        e.raw("let ");
        e.frag(&snippet.name);
        e.raw(" = |");
        e.frag(&snippet.params);
        e.raw("| __damask_krate::fragment(move |__damask: &mut dyn __damask_krate::Renderer| {\n");
    }
    e.at_line_start = false;
    emit_nodes(&snippet.body, Layout::ROOT, e)?;
    if snippet_awaits {
        e.raw("}) });\n");
    } else {
        e.raw("});\n");
    }
    Ok(())
}

fn emit_element(el: &Element, layout: Layout, e: &mut Emit) -> Result<(), String> {
    match el.kind {
        ElementKind::Html => emit_html_element(el, layout, e),
        ElementKind::Component => emit_component_element(el, layout, e),
        ElementKind::Slot => emit_slot_placeholder(el, layout, e),
    }
}

/// Append `__damask.write_raw("…")` for `raw`, if non-empty, and clear it.
fn flush_raw(raw: &mut String, e: &mut Emit) {
    if !raw.is_empty() {
        e.raw(&format!("__damask.write_raw({raw:?});\n"));
        raw.clear();
        e.at_line_start = false;
    }
}

fn emit_html_element(el: &Element, layout: Layout, e: &mut Emit) -> Result<(), String> {
    // Refused rather than written out as an attribute called `await`, which is what an author who
    // put the marker on the wrong tag would otherwise get: a page that renders and a component
    // above it that still will not compile.
    if crate::awaits::is_awaited(el) {
        return Err(format!(
            "`await` marks a *component* that renders asynchronously, and `<{}>` is an HTML \
             element",
            el.tag.as_str()
        ));
    }

    let mut raw = String::new();
    raw.push('<');
    raw.push_str(el.tag.as_str());

    // `class:name={cond}` directives override whatever `class` produces, so the
    // two cannot be emitted independently: they are collected and written once,
    // at the position `class` occupies (or the first directive's, if there is no
    // `class`). Without any directive a plain `class="…"` stays on the ordinary
    // path below, so the common case still lowers to literal text.
    let directives: Vec<&Attr> = el
        .attrs
        .iter()
        .filter(|a| a.name.as_str().starts_with("class:"))
        .collect();

    // The names this tag writes itself. A `{...}` spread on the same tag skips
    // them, so the element's own attributes win and the spread fills in the
    // rest — rather than the tag carrying `type` twice, which is not valid HTML
    // and which the browser settles by a rule nobody was thinking about.
    //
    // Decided here, when the template compiles, so what it costs at run time is
    // a scan of a list that is usually empty.
    let taken: Vec<&str> = el
        .attrs
        .iter()
        .map(|a| a.name.as_str())
        .filter(|name| !name.is_empty())
        .map(|name| name.strip_prefix("class:").map(|_| "class").unwrap_or(name))
        .collect();
    let taken = {
        let mut seen: Vec<&str> = Vec::new();
        for name in taken {
            if !seen.contains(&name) {
                seen.push(name);
            }
        }
        seen
    };

    for attr in &el.attrs {
        let name = attr.name.as_str();
        if name.starts_with("class:") {
            continue;
        }
        if name == "class"
            && (!directives.is_empty() || matches!(attr.value, AttrValue::Classes(_)))
        {
            flush_raw(&mut raw, e);
            emit_class_list(Some(&attr.value), &directives, e)?;
            continue;
        }
        // `data` expands into a run of `data-*` attributes — but only in the
        // forms that ask for it. A quoted `data="…"` stays the ordinary
        // attribute it has always been, which is what leaves `<object
        // data="movie.swf">` alone; a dynamic one there is written
        // `data="{self.url}"`.
        if name == "data" && matches!(attr.value, AttrValue::Data(_) | AttrValue::Expr(_)) {
            flush_raw(&mut raw, e);
            emit_data_set(&attr.value, e)?;
            continue;
        }
        match &attr.value {
            AttrValue::Boolean => {
                raw.push(' ');
                raw.push_str(attr.name.as_str());
            }
            // Only `class` parses into this, and only the branch above emits it.
            AttrValue::Classes(_) => {
                return Err(format!(
                    "`{name}` cannot take a class list; only `class` can"
                ));
            }
            // Likewise: only `data` parses into this.
            AttrValue::Data(_) => {
                return Err(format!("`{name}` cannot take a data map; only `data` can"));
            }
            AttrValue::Spread(code) => {
                require_expr(code.as_str(), "{...} attribute spread")?;
                flush_raw(&mut raw, e);
                let others: Vec<&str> =
                    taken.iter().copied().filter(|held| *held != name).collect();
                e.raw("__damask_krate::AttrSpread::write_attrs_except(&(");
                e.frag(code);
                e.raw(&format!("), &{others:?}, &mut *__damask);\n"));
            }
            AttrValue::Literal(parts) => {
                raw.push(' ');
                raw.push_str(attr.name.as_str());
                raw.push_str("=\"");
                for part in parts {
                    match part {
                        AttrPart::Text(t) => raw.push_str(t.as_str()),
                        AttrPart::Expr(code) => {
                            require_expr(code.as_str(), "attribute value")?;
                            flush_raw(&mut raw, e);
                            e.raw("__damask.write_escaped(__damask_krate::as_display(&(");
                            e.frag(code);
                            e.raw(")));\n");
                        }
                    }
                }
                raw.push('"');
            }
            // `name={expr}` defers the whole attribute to the value's type, so
            // a `bool` can render a bare `disabled` and an `Option` can decline
            // to render anything at all. That is why the name and quotes are
            // not written here: there may be nothing to write them around.
            AttrValue::Expr(code) => {
                require_expr(code.as_str(), "attribute value")?;
                flush_raw(&mut raw, e);
                e.raw("__damask_krate::Attr::write_attr(&(");
                e.frag(code);
                e.raw(&format!("), {:?}, &mut *__damask);\n", attr.name.as_str()));
            }
        }
    }

    // Directives with no `class` of their own to attach to.
    if !directives.is_empty() && !el.attrs.iter().any(|a| a.name.as_str() == "class") {
        flush_raw(&mut raw, e);
        emit_class_list(None, &directives, e)?;
    }

    if el.self_closing {
        if is_void_element(el.tag.as_str()) {
            raw.push('>');
        } else {
            raw.push_str(&format!("></{}>", el.tag.as_str()));
        }
        flush_raw(&mut raw, e);
        return Ok(());
    }

    raw.push('>');
    flush_raw(&mut raw, e);

    // Whitespace inside `<pre>` and friends is the reader's, so the renderer is
    // told to stop laying anything out until the end tag. The flag is set at run
    // time as well as honoured at compile time, because a component rendered in
    // here carries its own literals and knows nothing about where it landed.
    let verbatim = is_verbatim_element(el.tag.as_str());
    if verbatim {
        e.raw("__damask.set_verbatim(true);\n");
    }

    // A scope block so `{use}` (and bindings) are scoped to this element.
    e.raw("{\n");
    emit_nodes(&el.children, layout.inside(verbatim), e)?;
    e.raw("}\n");

    if verbatim {
        e.raw("__damask.set_verbatim(false);\n");
    } else {
        // The separator standing before this end tag was written for whatever
        // child came last — which may have been a `{#if}` that rendered
        // nothing, leaving the run before it, one level too deep. Which of
        // those happened is a run-time fact, so the tag states its own depth
        // and the renderer corrects what is there.
        e.raw(&format!("__damask.close_line({});\n", layout.depth));
    }

    e.raw(&format!(
        "__damask.write_raw({:?});\n",
        format!("</{}>", el.tag.as_str())
    ));
    e.at_line_start = false;
    Ok(())
}

/// `<slot/>` / `<slot name="x">fallback</slot>` — always a placeholder: render
/// what the caller passed for that slot, or the `<slot>`'s own body if unfilled.
fn emit_slot_placeholder(el: &Element, layout: Layout, e: &mut Emit) -> Result<(), String> {
    let name = slot_name(el)?;
    // The fill was written in the caller and laid out from *its* root, because
    // where it lands is this template's business, not the caller's — so the
    // slot's depth is added to it here. The fallback below is this template's
    // own markup and already carries that depth, so the two cannot share one
    // bracket: `Slots::render` applies the depth to whichever it takes.
    if e.is_async {
        e.raw(&format!(
            "__damask_slots.render_async({name:?}, &mut *__damask, {}, |__damask: &mut dyn __damask_krate::Renderer| {{ ::std::boxed::Box::pin(async move {{\n",
            layout.depth
        ));
        e.at_line_start = false;
        emit_nodes(&el.children, layout.same(), e)?;
        e.raw("}) }).await;\n");
    } else {
        e.raw(&format!(
            "__damask_slots.render({name:?}, &mut *__damask, {}, |__damask: &mut dyn __damask_krate::Renderer| {{\n",
            layout.depth
        ));
        e.at_line_start = false;
        emit_nodes(&el.children, layout.same(), e)?;
        e.raw("});\n");
    }
    e.at_line_start = false;
    Ok(())
}

/// Emit a Rust expression for a quoted attribute value: the literal itself when
/// it has no holes, a `format!` when it does.
fn emit_literal_string(parts: &[AttrPart], e: &mut Emit) -> Result<(), String> {
    if let [AttrPart::Text(t)] = parts {
        e.raw(&format!("{:?}", t.text));
        return Ok(());
    }
    let mut fmt = String::new();
    let mut args: Vec<&Spanned> = Vec::new();
    for part in parts {
        match part {
            AttrPart::Text(t) => fmt.push_str(&t.text.replace('{', "{{").replace('}', "}}")),
            AttrPart::Expr(code) => {
                require_expr(code.as_str(), "attribute value")?;
                fmt.push_str("{}");
                args.push(code);
            }
        }
    }
    e.raw(&format!("::std::format!({fmt:?}"));
    for arg in args {
        e.raw(", ");
        e.frag(arg);
    }
    e.raw(")");
    Ok(())
}

/// Emit the `class` attribute built from its value and any `class:` directives.
///
/// Everything lands in one [`damask::ClassList`], which dedupes and preserves
/// first-mention order — that is what lets a directive override the base list
/// rather than append a contradicting name after it.
fn emit_class_list(
    value: Option<&AttrValue>,
    directives: &[&Attr],
    e: &mut Emit,
) -> Result<(), String> {
    e.raw("{\nlet mut __damask_class = __damask_krate::ClassList::new();\n");

    match value {
        None => {}
        Some(AttrValue::Classes(terms)) => {
            for term in terms {
                match term {
                    ClassTerm::Nothing => {}
                    ClassTerm::Expr(code) => {
                        require_expr(code.as_str(), "class list entry")?;
                        e.raw("__damask_krate::ClassItem::add_to(&(");
                        e.frag(code);
                        e.raw("), &mut __damask_class);\n");
                    }
                    ClassTerm::Cond { name, when } => {
                        require_expr(when.as_str(), "class condition")?;
                        // Spliced bare, as `{#if}` does: parenthesising warns
                        // `unused_parens` in the user's crate, not in ours.
                        e.raw("if ");
                        e.frag(when);
                        e.raw(" { __damask_krate::ClassItem::add_to(&(");
                        e.frag(name);
                        e.raw("), &mut __damask_class); }\n");
                    }
                }
            }
        }
        Some(AttrValue::Literal(parts)) => {
            e.raw("__damask_krate::ClassItem::add_to(&(");
            emit_literal_string(parts, e)?;
            e.raw("), &mut __damask_class);\n");
        }
        Some(AttrValue::Expr(code)) => {
            require_expr(code.as_str(), "class")?;
            e.raw("__damask_krate::ClassItem::add_to(&(");
            e.frag(code);
            e.raw("), &mut __damask_class);\n");
        }
        Some(AttrValue::Boolean) => return Err("`class` needs a value".into()),
        // A spread carries its own names, so it never reaches here as `class`.
        Some(AttrValue::Spread(_)) => unreachable!("a spread has no attribute name"),
        // Only an attribute named `data` parses into this, and it is routed to
        // `emit_data_set` before it could reach here.
        Some(AttrValue::Data(_)) => unreachable!("a data map is not a class value"),
    }

    // Applied after the base list, because that is what "takes precedence"
    // means: the directive is the last word on whether its class is there.
    for attr in directives {
        let name = &attr.name.as_str()["class:".len()..];
        if name.is_empty() {
            return Err("`class:` needs a class name after the colon".into());
        }
        e.raw(&format!("__damask_class.set({name:?}, "));
        match &attr.value {
            AttrValue::Boolean => e.raw("true"),
            AttrValue::Expr(code) => {
                require_expr(code.as_str(), "class directive")?;
                e.frag(code);
            }
            _ => {
                return Err(format!(
                    "`class:{name}` takes a boolean expression, as `class:{name}={{…}}`"
                ));
            }
        }
        e.raw(");\n");
    }

    e.raw("__damask_class.write_attr(\"class\", &mut *__damask);\n}\n");
    Ok(())
}

/// Emit the run of `data-*` attributes a `data` value expands into.
///
/// Everything lands in one [`damask::DataSet`], for the reason the class forms
/// share one `ClassList`: a key mentioned twice has to resolve to one
/// attribute, and only a collector that outlives the individual entries can
/// decide which mention wins. Sibling `data-*` attributes written out longhand
/// are *not* collected here — they stay on the ordinary `Attr` path, so their
/// values are held to `Attr` rather than to `DataValue`, and a `data` map
/// appearing next to one cannot change how it compiles.
fn emit_data_set(value: &AttrValue, e: &mut Emit) -> Result<(), String> {
    e.raw("{\nlet mut __damask_data = __damask_krate::DataSet::new();\n");

    match value {
        AttrValue::Data(terms) => {
            for term in terms {
                match term {
                    DataTerm::Nothing => {}
                    DataTerm::Expr(code) => {
                        require_expr(code.as_str(), "data list entry")?;
                        e.raw("__damask_krate::DataItem::add_to(&(");
                        e.frag(code);
                        e.raw("), &mut __damask_data);\n");
                    }
                    DataTerm::Pair { key, value } => {
                        require_expr(value.as_str(), "data value")?;
                        e.raw("__damask_krate::DataValue::add_to(&(");
                        e.frag(value);
                        e.raw("), ");
                        // The key is spliced as the Rust it was written as — a
                        // string literal — so that it is checked, and spanned,
                        // like every other fragment.
                        e.frag(key);
                        e.raw(", &mut __damask_data);\n");
                    }
                }
            }
        }
        AttrValue::Expr(code) => {
            require_expr(code.as_str(), "data")?;
            e.raw("__damask_krate::DataItem::add_to(&(");
            e.frag(code);
            e.raw("), &mut __damask_data);\n");
        }
        // A quoted `data="…"` and a bare `data` never reach here: both stay
        // ordinary attributes, and a spread has no name to be `data`.
        _ => unreachable!("only a braced or bracketed `data` value reaches a data set"),
    }

    e.raw("__damask_data.write_attrs(&mut *__damask);\n}\n");
    Ok(())
}

/// A `<slot>`'s name: the `name="…"` attribute, or [`DEFAULT_SLOT`] (empty) for
/// the unnamed default slot.
///
/// [`DEFAULT_SLOT`]: https://docs.rs/damask/latest/damask/constant.DEFAULT_SLOT.html
fn slot_name(el: &Element) -> Result<String, String> {
    match el.attrs.iter().find(|a| a.name == "name") {
        None => Ok(String::new()),
        // A slot name is resolved at compile time, so it must be one static
        // piece — an interpolated one would name a different slot per render.
        Some(attr) => match &attr.value {
            AttrValue::Literal(parts) => match parts.as_slice() {
                [AttrPart::Text(name)] if name.text.is_empty() => Err(
                    "`<slot name>` must not be empty; write `<slot/>` for the default slot".into(),
                ),
                [AttrPart::Text(name)] => Ok(name.text.clone()),
                _ => Err("`<slot name>` must be a plain string literal".into()),
            },
            _ => Err("`<slot name>` must be a string literal".into()),
        },
    }
}

/// The slot a direct child of a component element fills — its `slot="…"`
/// attribute — or `None` for content that belongs to the default slot.
///
/// The attribute is consumed here: it is a routing instruction for the enclosing
/// component, so it reaches neither the rendered markup nor the child's props.
/// Anywhere else it is an ordinary attribute, which is what lets a template emit
/// a real `slot=` for a browser-side custom element.
fn fill_name(el: &Element) -> Result<Option<String>, String> {
    let Some(attr) = el.attrs.iter().find(|a| a.name == "slot") else {
        return Ok(None);
    };
    // Resolved at compile time, for the same reason `<slot name>` is: an
    // interpolated value would fill a different slot per render.
    match &attr.value {
        AttrValue::Literal(parts) => match parts.as_slice() {
            [AttrPart::Text(name)] if name.text.is_empty() => Err(
                "`slot` must not be empty; content with no `slot` fills the default slot".into(),
            ),
            [AttrPart::Text(name)] => Ok(Some(name.text.clone())),
            _ => Err("`slot` must be a plain string literal".into()),
        },
        _ => Err("`slot` must be a string literal".into()),
    }
}

/// `<Comp attr={e}>…</Comp>` — build `Comp { attr: e }` and render it with the
/// element's content as its slot fills.
fn emit_component_element(el: &Element, layout: Layout, e: &mut Emit) -> Result<(), String> {
    // Partition children into named-slot fills and default-slot content, as the
    // DOM does: a direct child carrying `slot="x"` fills `x` — the element
    // itself, not just its content — and several children may name the same
    // slot, in which case they land there in document order. Everything else,
    // including a bare `<slot/>` placeholder that forwards this component's own
    // default slot, is default-slot content.
    let mut default: Vec<&Node> = Vec::new();
    let mut named: Vec<(String, Vec<Node>)> = Vec::new();
    for child in &el.children {
        let Node::Element(child_el) = child else {
            default.push(child);
            continue;
        };
        let Some(name) = fill_name(child_el)? else {
            default.push(child);
            continue;
        };
        let mut routed = child_el.clone();
        routed.attrs.retain(|a| a.name != "slot");
        let routed = Node::Element(routed);
        match named.iter_mut().find(|(seen, _)| *seen == name) {
            Some((_, body)) => body.push(routed),
            None => named.push((name, vec![routed])),
        }
    }

    // Default slot: filled only when there is real (non-whitespace) content.
    let has_default = default
        .iter()
        .any(|n| !matches!(n, Node::Text(t) if t.as_str().trim().is_empty()));

    // The fills borrow temporaries that live to the end of this statement, so
    // slot content stays on the stack and can borrow the enclosing scope.
    let has_slots = has_default || !named.is_empty();
    let (trait_name, method) = match (e.is_async, has_slots) {
        (false, false) => ("Render", "render_into"),
        (false, true) => ("Render", "render_slots"),
        (true, false) => ("AsyncRender", "render_into_async"),
        (true, true) => ("AsyncRender", "render_slots_async"),
    };
    // The component's markup is laid out from its own root, so the depth of
    // this call site is what places it. The bracket spans the whole statement
    // rather than just the call, because the slot fills below are closures the
    // callee invokes while it runs — they need the same depth, plus the one the
    // callee's `<slot>` adds.
    if layout.depth > 0 {
        e.raw(&format!("__damask.push_indent({});\n", layout.depth));
    }
    e.raw(&format!("__damask_krate::{trait_name}::{method}(&("));
    // Built through the derive's hidden builder rather than as a struct literal:
    // the props named here are the ones the author wrote, and only the derive
    // knows which of the rest may be skipped and what they default to. A prop
    // left out that cannot be is a trait-bound error naming it.
    //
    // The tag name and each attribute name are spliced as *mapped* fragments:
    // they land on the struct name and its per-prop setters, which the derive
    // spans onto the fields, so the language server can answer hover and
    // go-to-definition over `<Comp attr=…>` itself, not just the Rust inside the
    // attribute values.
    e.frag(&el.tag);
    e.raw("::__damask_props()\n");

    for attr in &el.attrs {
        // The `await` marker is not a prop: it is the author telling this pass what it cannot
        // see — that this component renders asynchronously — so that the call goes through the
        // async path. `await` is a keyword, so it could not name a prop in any case.
        if attr.name.as_str() == "await" && matches!(attr.value, AttrValue::Boolean) {
            continue;
        }

        // A name that could not be a method — everything hyphenated, and every
        // Rust keyword — has no setter to resolve against and no fallback trait
        // to route it, so it is written straight to the component's bag. A name
        // that *could* be a prop falls through to the setter calls below, where
        // method resolution decides between the two.
        if !matches!(attr.value, AttrValue::Spread(_))
            && !crate::rest::could_be_a_prop(attr.name.as_str())
        {
            let name = attr.name.as_str();
            match &attr.value {
                AttrValue::Expr(code) => {
                    require_expr(code.as_str(), "attribute value")?;
                    e.raw(&format!(".__damask_rest_any({name:?}, ("));
                    e.frag(code);
                    e.raw("))\n");
                }
                AttrValue::Literal(parts) => {
                    // Static text on both sides stays borrowed; an interpolated
                    // value is a `String` and reaches the same bag as a value.
                    match parts.as_slice() {
                        [AttrPart::Text(_)] => {
                            e.raw(&format!(".__damask_rest_static_any({name:?}, "));
                            emit_literal_string(parts, e)?;
                            e.raw(")\n");
                        }
                        _ => {
                            e.raw(&format!(".__damask_rest_any({name:?}, "));
                            emit_literal_string(parts, e)?;
                            e.raw(")\n");
                        }
                    }
                }
                AttrValue::Boolean => {
                    e.raw(&format!(".__damask_rest_bare_any({name:?})\n"));
                }
                AttrValue::Classes(_) | AttrValue::Data(_) => {
                    return Err(format!(
                        "`{name}` is an attribute this component does not name, so it takes a \
                         value rather than a class list or a data map"
                    ));
                }
                // Unreachable: a spread is excluded by the guard above,
                // since it carries names rather than one.
                AttrValue::Spread(_) => unreachable!("a spread has no attribute name"),
            }
            continue;
        }

        match &attr.value {
            AttrValue::Expr(code) => {
                require_expr(code.as_str(), "attribute value")?;
                e.raw(".");
                e.frag(&attr.name);
                e.raw("((");
                e.frag(code);
                e.raw("))\n");
            }
            // A quoted value lands on a prop, so it must be a `String`-ish value
            // rather than markup: an interpolating one is formatted, and a plain
            // one stays the literal it was.
            //
            // Both convert against the prop's type, but only one can do it
            // through `Into`: an interpolated value is a `String`, which reaches
            // an `Option<String>` prop as readily as a `String` one, while
            // static text is a `&'static str`, which reaches no `Option` at all.
            // `props::literal` is that missing step, and infers which it needs
            // from the prop.
            AttrValue::Literal(parts) => {
                let interpolating = !matches!(parts.as_slice(), [AttrPart::Text(_)]);
                // Static text goes to the prop's *literal* setter, which knows
                // what it is converting into; an interpolated value is already a
                // `String` and reaches the ordinary one.
                e.raw(if interpolating {
                    ".__damask_text_"
                } else {
                    ".__damask_literal_"
                });
                e.frag(&attr.name);
                e.raw("(");
                emit_literal_string(parts, e)?;
                e.raw(")\n");
            }
            // Bare `flag`, which is how `flag={true}` is written when the value
            // is the point. A skippable prop's setter takes the value or its
            // `Option`, so this reaches an `Option<bool>` prop as readily as a
            // `bool` one with nothing written around it.
            AttrValue::Boolean => {
                e.raw(".");
                e.frag(&attr.name);
                e.raw("(true)\n");
            }
            // A class list assembles markup, and a component prop is a value.
            // `class={…}` with an ordinary expression is the way to pass one.
            AttrValue::Classes(_) => {
                return Err(format!(
                    "`{}` is a component prop, so it cannot take a class list",
                    attr.name.as_str()
                ));
            }
            // The same, and the reason is worth stating: on a component `data`
            // is an ordinary prop, so `data={expr}` there passes the value
            // through untouched. It is the map and list forms that assemble
            // markup, and those have nowhere to go.
            AttrValue::Data(_) => {
                return Err(format!(
                    "`{}` is a component prop, so it cannot take a data map",
                    attr.name.as_str()
                ));
            }
            // A spread carries names rather than one, so there is no prop it
            // could be. It goes to the bag whole, folded in where it was
            // written — which is how a set assembled in Rust, or forwarded from
            // an enclosing component, reaches a call site that cannot name it.
            AttrValue::Spread(code) => {
                require_expr(code.as_str(), "{...} attribute spread")?;
                e.raw(".__damask_rest_spread_any(&(");
                e.frag(code);
                e.raw("))\n");
            }
        }
    }

    e.raw(".__damask_build()), &mut *__damask");

    if !has_slots {
        e.raw(")");
        if e.is_async {
            e.raw(".await");
        }
        e.raw(";\n");
        e.at_line_start = false;
        if layout.depth > 0 {
            e.raw(&format!("__damask.pop_indent({});\n", layout.depth));
        }
        return Ok(());
    }

    e.raw(", __damask_krate::Slots::new(&[\n");
    // Each fill is judged on its own body, not on the enclosing template: a
    // fill that awaits nothing stays a plain `Fragment` even inside a template
    // that awaits elsewhere, so it costs no boxed future. Only the one that
    // genuinely suspends becomes an `AsyncFragment`, which is a shape the
    // callee never sees — it renders `<slot/>` the way it always did.
    let outer_is_async = e.is_async;
    if has_default {
        let awaits = default.iter().any(|n| crate::awaits::node_needs_async(n));
        e.is_async = awaits;
        e.raw(&open_fill("__damask_krate::DEFAULT_SLOT", awaits));
        e.at_line_start = false;
        for (i, n) in default.iter().enumerate() {
            emit_node(n, Layout::ROOT, i + 1 == default.len(), e)?;
        }
        e.raw(close_fill(awaits));
    }
    for (name, body) in &named {
        let awaits = crate::awaits::nodes_need_async(body);
        e.is_async = awaits;
        e.raw(&open_fill(&format!("{name:?}"), awaits));
        e.at_line_start = false;
        emit_nodes(body, Layout::ROOT, e)?;
        e.raw(close_fill(awaits));
    }
    e.is_async = outer_is_async;
    e.raw("]))");
    if e.is_async {
        e.raw(".await");
    }
    e.raw(";\n");
    e.at_line_start = false;
    if layout.depth > 0 {
        e.raw(&format!("__damask.pop_indent({});\n", layout.depth));
    }
    Ok(())
}

/// The opening of one slot fill: a `Slot` holding a fragment, boxed-future
/// shaped when the markup inside it suspends.
fn open_fill(name: &str, awaits: bool) -> String {
    match awaits {
        false => format!(
            "__damask_krate::Slot::new({name}, &__damask_krate::fragment(|__damask: &mut dyn __damask_krate::Renderer| {{\n"
        ),
        true => format!(
            "__damask_krate::Slot::new_async({name}, &__damask_krate::fragment_async(|__damask: &mut dyn __damask_krate::Renderer| {{ ::std::boxed::Box::pin(async move {{\n"
        ),
    }
}

fn close_fill(awaits: bool) -> &'static str {
    match awaits {
        false => "})),\n",
        true => "}) })),\n",
    }
}

fn require_expr(code: &str, tag: &str) -> Result<(), String> {
    if code.trim().is_empty() {
        Err(format!("empty expression in `{tag}`"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{is_statement, lower, lower_mapped};

    fn body(src: &str) -> String {
        lower(&crate::parse(src).unwrap()).unwrap()
    }

    /// The template substrings that lowering recorded as verbatim fragments.
    fn mapped(src: &str) -> Vec<&str> {
        let (out, map) = lower_mapped(&crate::parse(src).unwrap()).unwrap();
        for m in &map.mappings {
            assert_eq!(
                &src[m.source.start..m.source.end],
                &out[m.generated.start..m.generated.end],
                "mapping must cover byte-identical text",
            );
        }
        map.mappings
            .iter()
            .map(|m| &src[m.source.start..m.source.end])
            .collect()
    }

    /// A component's name and attribute names are mapped, so the language
    /// server can resolve `<Comp attr=…>` itself — not only the Rust inside the
    /// attribute values.
    #[test]
    fn component_name_and_attribute_names_are_mapped() {
        let frags = mapped(r#"<Card title={self.t} label="hi" flag/>"#);
        assert!(
            frags.contains(&"Card"),
            "component name unmapped: {frags:?}"
        );
        assert!(frags.contains(&"title"), "expr attr unmapped: {frags:?}");
        assert!(frags.contains(&"label"), "literal attr unmapped: {frags:?}");
        assert!(frags.contains(&"flag"), "boolean attr unmapped: {frags:?}");
    }

    /// HTML tag and attribute names are *not* mapped: they stay markup, owned by
    /// the HTML language server, and have no Rust to resolve to.
    #[test]
    fn html_names_are_not_mapped() {
        let frags = mapped(r#"<div class="x" id={self.id}>hi</div>"#);
        assert!(!frags.contains(&"div"), "html tag mapped: {frags:?}");
        assert!(!frags.contains(&"class"), "html attr mapped: {frags:?}");
        assert!(!frags.contains(&"id"), "html attr mapped: {frags:?}");
        // The Rust inside an attribute value is still mapped.
        assert!(frags.contains(&"self.id"), "attr value unmapped: {frags:?}");
    }

    #[test]
    fn text_and_expression() {
        let b = body("Hi {self.name}!");
        assert!(b.contains(r#"__damask.write_text("Hi ")"#));
        assert!(b.contains("::damask::splice(&(self.name), &mut *__damask)"));
        assert!(b.contains(r#"__damask.write_text("!")"#));
    }

    #[test]
    fn block_value_vs_statement() {
        assert!(is_statement("let x = 5"));
        assert!(is_statement("const A: u32 = 5"));
        assert!(is_statement("foo();"));
        assert!(!is_statement("2 + 3; 10"));
        assert!(!is_statement("self.name"));
        assert!(!is_statement("letter")); // not the `let` keyword
        assert!(body("{let x = 5}").contains("let x = 5;"));
        assert!(body("{2 + 3; 10}").contains("::damask::splice(&({ 2 + 3; 10 }), &mut *__damask)"));
    }

    #[test]
    fn directives_and_use() {
        assert!(
            body("{@html self.body}")
                .contains("write_display_raw(::damask::as_display(&(self.body)))")
        );
        assert!(
            body("{@render self.footer}").contains("::damask::Render::render_into(&(self.footer)")
        );
        assert!(body("{use crate::Card}").contains("use crate::Card;"));
    }

    #[test]
    fn if_and_for() {
        let b = body("{#if self.a}x{:else}y{/if}");
        assert!(b.contains("if self.a {"));
        assert!(b.contains("} else {"));
        assert!(
            body("{#for item in &self.items}{item}{/for}").contains("for item in &self.items {")
        );
        // The header is Rust verbatim: a tuple pattern over `.enumerate()` passes
        // straight through, with no lowering-side rewrite.
        assert!(
            body("{#for (i, item) in self.items.iter().enumerate()}{i}{/for}")
                .contains("for (i, item) in self.items.iter().enumerate() {")
        );
    }

    #[test]
    fn html_element_scopes_and_attrs() {
        let b = body(r#"<div id={self.id}>{use crate::X}hi</div>"#);
        // `name={expr}` defers to the value's type, which is what lets it
        // render nothing at all.
        assert!(b.contains(r#"::damask::Attr::write_attr(&(self.id), "id", &mut *__damask);"#));
        // element content is a scope block containing the use
        assert!(b.contains("use crate::X;"));
        assert!(b.contains(r#"write_raw("</div>")"#));
    }

    #[test]
    fn quoted_attribute_values_interpolate() {
        let b = body(r#"<div title="a {self.x} b"></div>"#);
        assert!(b.contains(r#"write_raw("<div title=\"a ")"#));
        assert!(b.contains("__damask.write_escaped(::damask::as_display(&(self.x)))"));
        assert!(b.contains(r#"write_raw(" b\">")"#));
        // A value with no holes stays literal text, not a format!.
        assert!(body(r#"<div title="plain"></div>"#).contains(r#" title=\"plain\""#));
    }

    /// An interpolating value lands on a component *prop*, so it must be an
    /// owned `String` expression and not a borrow of one — the class-list path
    /// wraps its argument in `&(…)` itself, and an extra one there was absorbed
    /// by the blanket impl rather than reported.
    ///
    /// It goes to the prop's own text setter rather than through `Into`, which
    /// is what tells `Option<String>` from `String` — see `props`.
    #[test]
    fn interpolating_value_on_a_component_prop_is_owned() {
        let b = body(r#"<Comp class="a {self.x} b"/>"#);
        assert!(
            b.contains(r#".__damask_text_class(::std::format!("a {} b", self.x))"#),
            "{b}"
        );
    }

    #[test]
    fn class_list_and_map_forms() {
        let b = body(r#"<div class=[Some("a"), None, "b", { "c": self.on }]></div>"#);
        assert!(b.contains("let mut __damask_class = ::damask::ClassList::new();"));
        assert!(b.contains(r#"::damask::ClassItem::add_to(&(Some("a")), &mut __damask_class);"#));
        // A literal `None` contributes nothing and is not emitted at all: it
        // has no type to infer, so it cannot be lowered as an expression.
        assert!(!b.contains("None"));
        assert!(b.contains(r#"if self.on { ::damask::ClassItem::add_to(&("c")"#));

        let m = body(r#"<div class={ "c": self.on, "d": !self.on }></div>"#);
        assert!(m.contains(r#"if self.on { ::damask::ClassItem::add_to(&("c")"#));
        assert!(m.contains(r#"if !self.on { ::damask::ClassItem::add_to(&("d")"#));
    }

    #[test]
    fn class_brace_disambiguates_map_from_expression() {
        // No top-level colon: an ordinary Rust expression, not a map.
        let e = body(r#"<div class={self.class()}></div>"#);
        assert!(e.contains(r#"::damask::Attr::write_attr(&(self.class()), "class""#));
        // A `::` path inside is not a colon for these purposes.
        let p = body(r#"<div class={ "c": matches!(self.t, Tone::Ok) }></div>"#);
        assert!(p.contains("__damask_class"));
    }

    #[test]
    fn data_expression_list_and_map_forms() {
        let e = body(r#"<div data={self.wiring}></div>"#);
        assert!(e.contains("let mut __damask_data = ::damask::DataSet::new();"));
        assert!(e.contains("::damask::DataItem::add_to(&(self.wiring), &mut __damask_data);"));
        assert!(e.contains("__damask_data.write_attrs(&mut *__damask);"));

        let l = body(r#"<div data=[self.base(), None, { "open": self.on }]></div>"#);
        assert!(l.contains("::damask::DataItem::add_to(&(self.base()), &mut __damask_data);"));
        // A literal `None` drops out at compile time, as it does in a class list.
        assert!(!l.contains("None"));
        assert!(
            l.contains(r#"::damask::DataValue::add_to(&(self.on), "open", &mut __damask_data);"#)
        );

        let m = body(r#"<div data={ "controller": "modal", "index": self.i }></div>"#);
        assert!(m.contains(
            r#"::damask::DataValue::add_to(&("modal"), "controller", &mut __damask_data);"#
        ));
        assert!(
            m.contains(r#"::damask::DataValue::add_to(&(self.i), "index", &mut __damask_data);"#)
        );
    }

    /// A quoted `data="…"` is the ordinary attribute it has always been, which
    /// is what leaves `<object data="movie.swf">` compiling to literal text.
    #[test]
    fn a_quoted_data_value_stays_an_ordinary_attribute() {
        let b = body(r#"<object data="movie.swf"></object>"#);
        assert!(b.contains(r#"<object data=\"movie.swf\""#));
        assert!(!b.contains("DataSet"));
    }

    /// Longhand `data-*` attributes are not collected into the set: they stay on
    /// the `Attr` path whether or not a `data` map sits beside them, so adding
    /// one cannot change how the other compiles.
    #[test]
    fn longhand_data_attributes_are_left_alone() {
        let b = body(r#"<div data-controller="modal" data={self.extra}></div>"#);
        assert!(b.contains(r#"data-controller=\"modal\""#));
        assert!(b.contains("::damask::DataItem::add_to(&(self.extra), &mut __damask_data);"));
    }

    #[test]
    fn a_data_map_is_an_error_off_data() {
        // Only `data` parses into a data map, and only on an HTML element.
        assert!(lower(&crate::parse(r#"<Comp data={ "a": self.x }/>"#).unwrap()).is_err());
        assert!(lower(&crate::parse(r#"<Comp data=[self.x]/>"#).unwrap()).is_err());
        // On a component, a plain `data={expr}` is an ordinary prop.
        let p = body(r#"<Comp data={self.x}/>"#);
        assert!(p.contains(".data((self.x))"));
    }

    #[test]
    fn attribute_spread() {
        let b = body(r#"<div {...self.extra}></div>"#);
        assert!(
            b.contains(
                "::damask::AttrSpread::write_attrs_except(&(self.extra), &[], &mut *__damask);"
            ),
            "{b}"
        );
        // The element's own attributes win, so the spread is told to skip them.
        let held = body(r#"<input type="text" {...self.extra}>"#);
        assert!(
            held.contains(r#"write_attrs_except(&(self.extra), &["type"], &mut *__damask);"#),
            "{held}"
        );
        // A `class:` directive is still `class`, and only names it once.
        let classes = body(r#"<div class="a" class:b={self.on} {...self.extra}></div>"#);
        assert!(
            classes.contains(r#"write_attrs_except(&(self.extra), &["class"], &mut *__damask);"#),
            "{classes}"
        );
        // On a component the same spread is a whole set for its bag, since a
        // spread carries names rather than one and so can be no single prop.
        let c = body(r#"<Comp {...self.extra}/>"#);
        assert!(c.contains(".__damask_rest_spread_any(&(self.extra))"));
    }

    /// A hyphenated name could never be a method, so it needs no fallback and
    /// is written to the bag where it stands.
    #[test]
    fn an_attribute_a_component_cannot_name_goes_straight_to_its_bag() {
        let b = body(r#"<Hidden data-cover-target="input"/>"#);
        assert!(
            b.contains(r#".__damask_rest_static_any("data-cover-target", "input")"#),
            "{b}"
        );
        assert!(
            !b.contains("__DamaskRest_data"),
            "no fallback trait is needed"
        );
    }

    /// A keyword is the other name no field can carry, and `type` is the one
    /// that matters — it is why a control's prop had to be called `kind`.
    #[test]
    fn a_keyword_attribute_goes_to_the_bag_too() {
        let b = body(r#"<TextInput type="email" for={self.id()} async/>"#);
        assert!(
            b.contains(r#".__damask_rest_static_any("type", "email")"#),
            "{b}"
        );
        assert!(
            b.contains(r#".__damask_rest_any("for", (self.id()))"#),
            "{b}"
        );
        assert!(b.contains(r#".__damask_rest_bare_any("async")"#), "{b}");
    }

    /// An ident-shaped name might be a prop and might not, and this pass cannot
    /// tell — so it emits the setter call *and* the fallback that catches it.
    #[test]
    fn an_ident_shaped_attribute_is_left_to_method_resolution() {
        let b = body(r#"<TextInput autofocus placeholder="mail" rows={4}/>"#);
        assert!(b.contains(".autofocus(true)"), "{b}");
        assert!(
            b.contains(r#".__damask_literal_placeholder("mail")"#),
            "{b}"
        );
        assert!(b.contains(".rows((4))"), "{b}");
        for name in ["autofocus", "placeholder", "rows"] {
            assert!(
                b.contains(&format!("trait __DamaskRest_{name}")),
                "{name}: {b}"
            );
            assert!(
                b.contains(&format!(
                    "impl<__DamaskAny> __DamaskRest_{name} for __DamaskAny"
                )),
                "{name}: {b}"
            );
        }
        // An element's attributes are markup and never props, so they bring no
        // fallback with them.
        let plain = body(r#"<input placeholder="mail">"#);
        assert!(!plain.contains("__DamaskRest_placeholder"), "{plain}");
    }

    /// One trait per name, however many call sites write it — a duplicate
    /// definition in the same block would not compile.
    #[test]
    fn a_name_written_twice_defines_one_fallback() {
        let b = body(r#"<A gap={1}/><B gap={2}/>"#);
        assert_eq!(b.matches("trait __DamaskRest_gap").count(), 1, "{b}");
    }

    #[test]
    fn class_directives_take_precedence() {
        let b = body(r#"<div class="a b" class:b={self.off} class:c></div>"#);
        assert!(b.contains(r#"::damask::ClassItem::add_to(&("a b"), &mut __damask_class);"#));
        assert!(b.contains(r#"__damask_class.set("b", self.off);"#));
        assert!(b.contains(r#"__damask_class.set("c", true);"#));
        // The whole thing is written once, after the directives are applied.
        assert!(b.contains(r#"__damask_class.write_attr("class", &mut *__damask);"#));
        // A directive with no `class` of its own still produces the attribute.
        assert!(body(r#"<div class:c={self.on}></div>"#).contains("__damask_class"));
    }

    #[test]
    fn let_and_use_are_element_scoped() {
        // Both a `{let}` and a `{use}` inside <div> land inside the element's
        // scope block (between the `<div>` write and the `</div>` write), so
        // they are not visible after `</div>`.
        let b = body(r#"<div>{let x = 5}{use crate::X}{x}</div>"#);
        let open = b.find(r#"write_raw("<div>")"#).unwrap();
        let block_open = b[open..].find('{').unwrap() + open;
        let close = b.find(r#"write_raw("</div>")"#).unwrap();
        let let_pos = b.find("let x = 5;").unwrap();
        let use_pos = b.find("use crate::X;").unwrap();
        // both statements sit strictly between the block open and the close tag
        assert!(block_open < let_pos && let_pos < close);
        assert!(block_open < use_pos && use_pos < close);
    }

    #[test]
    fn void_and_self_closing_elements() {
        assert!(body("<br>").contains(r#"write_raw("<br>")"#));
        assert!(body("<hr class=\"x\">").contains(r#"write_raw("<hr class=\"x\">")"#));
    }

    #[test]
    fn component_element_construction() {
        let b = body(r#"<Card title={2 + 8} tag="h1">body<p slot="foot">f</p></Card>"#);
        assert!(b.contains("::damask::Render::render_slots(&(Card::__damask_props()"));
        assert!(b.contains(".title((2 + 8))"));
        assert!(b.contains(r#".__damask_literal_tag("h1")"#));
        assert!(b.contains(".__damask_build())"));
        assert!(b.contains("::damask::Slot::new(::damask::DEFAULT_SLOT, &::damask::fragment("));
        assert!(b.contains(r#"::damask::Slot::new("foot", &::damask::fragment("#));
    }

    #[test]
    fn component_element_without_content_skips_slots() {
        // Nothing to fill: no slot slice is built, and the plain render path is
        // used — same call `{@render …}` emits.
        let b = body(r#"<Card title="x"/>"#);
        assert!(b.contains("::damask::Render::render_into(&(Card::__damask_props()"));
        assert!(!b.contains("::damask::Slots::new"));
    }

    #[test]
    fn slot_placeholder_resolves_against_the_caller_slots() {
        assert!(body("<slot/>").contains(r#"__damask_slots.render("", &mut *__damask"#));
        assert!(
            body(r#"<slot name="foot"/>"#)
                .contains(r#"__damask_slots.render("foot", &mut *__damask"#)
        );
    }

    #[test]
    fn slot_fallback_body_is_the_unfilled_branch() {
        let b = body(r#"<slot name="foot">fallback</slot>"#);
        let call = b.find(r#"__damask_slots.render("foot""#).unwrap();
        let fallback = b.find(r#"write_text("fallback")"#).unwrap();
        assert!(call < fallback, "fallback body belongs to the closure");
    }

    #[test]
    fn a_fill_carries_the_element_that_named_it() {
        // Web-component semantics: the whole `<p>` lands in the slot, and the
        // `slot` attribute that routed it there is not part of the markup.
        let b = body(r#"<Card><p slot="foot">f</p></Card>"#);
        let fill = b.find(r#"::damask::Slot::new("foot""#).unwrap();
        let open = b.find(r#"write_raw("<p>")"#).unwrap();
        assert!(fill < open, "the element belongs to the fill: {b}");
        assert!(!b.contains("slot=\\\""), "`slot` leaked into markup: {b}");
    }

    #[test]
    fn several_children_can_name_the_same_slot() {
        let b = body(r#"<Card><p slot="foot">1</p><i>x</i><p slot="foot">2</p></Card>"#);
        assert_eq!(
            b.matches(r#"::damask::Slot::new("foot""#).count(),
            1,
            "one fill, not two: {b}"
        );
        let fill = b.find(r#"::damask::Slot::new("foot""#).unwrap();
        let first = b.find(r#"write_text("1")"#).unwrap();
        let second = b.find(r#"write_text("2")"#).unwrap();
        let default = b.find(r#"write_text("x")"#).unwrap();
        assert!(fill < first && first < second, "not in order: {b}");
        assert!(default < fill, "unslotted content is not the fill: {b}");
    }

    #[test]
    fn a_slot_placeholder_can_be_routed_into_a_fill() {
        // `<slot name="foot" slot="foot"/>` forwards: the placeholder resolves
        // against *this* component's caller, and lands in the child's `foot`.
        let b = body(r#"<Card><slot name="foot" slot="foot"/></Card>"#);
        let fill = b.find(r#"::damask::Slot::new("foot""#).unwrap();
        let forward = b.find(r#"__damask_slots.render("foot""#).unwrap();
        assert!(fill < forward, "placeholder sits inside the fill: {b}");
    }

    #[test]
    fn bare_slot_in_a_component_forwards_the_default_slot() {
        // No `slot` attribute, so it is ordinary default-slot content — and
        // being a placeholder, it forwards this component's own default slot.
        let b = body("<Card><slot/></Card>");
        let fill = b
            .find("::damask::Slot::new(::damask::DEFAULT_SLOT")
            .unwrap();
        let forward = b.find(r#"__damask_slots.render("""#).unwrap();
        assert!(
            fill < forward,
            "forwarding placeholder sits inside the fill"
        );
    }

    #[test]
    fn a_forwarded_slot_mixes_with_other_content() {
        let b = body("<Card>before<slot/>after</Card>");
        let before = b.find(r#"write_text("before")"#).unwrap();
        let forward = b.find(r#"__damask_slots.render("""#).unwrap();
        let after = b.find(r#"write_text("after")"#).unwrap();
        assert!(before < forward && forward < after, "order not preserved");
    }

    /// The caller's fills are reachable from any `{ … }` tag, so a template can
    /// guard on one — which `<slot>` alone cannot do, a fallback standing in for
    /// the content rather than the markup around it.
    #[test]
    fn slots_are_bound_for_template_expressions() {
        let b = body(r#"{#if slots.has("foot")}<footer>{@render slots.get("foot")}</footer>{/if}"#);
        assert!(b.contains("let slots = __damask_slots;"), "unbound: {b}");
        assert!(b.contains(r#"if slots.has("foot")"#), "unexpected: {b}");
        assert!(
            b.contains(r#"::damask::Render::render_into(&(slots.get("foot"))"#),
            "unexpected: {b}"
        );
    }

    #[test]
    fn empty_slot_name_is_an_error() {
        let err = lower(&crate::parse(r#"<slot name=""/>"#).unwrap()).unwrap_err();
        assert!(err.contains("must not be empty"), "unexpected: {err}");
    }

    #[test]
    fn empty_fill_name_is_an_error() {
        let err = lower(&crate::parse(r#"<Card><p slot="">x</p></Card>"#).unwrap()).unwrap_err();
        assert!(err.contains("must not be empty"), "unexpected: {err}");
    }

    /// `slot` only routes content inside a component element. Everywhere else it
    /// is an ordinary attribute, so a template can address a browser-side
    /// custom element's shadow slots.
    #[test]
    fn slot_outside_a_component_stays_an_attribute() {
        let b = body(r#"<my-card><p slot="foot">f</p></my-card>"#);
        assert!(b.contains(r#"<p slot=\"foot\">"#), "unexpected: {b}");
    }

    #[test]
    fn empty_tag_is_an_error() {
        // `{ }` is rejected at parse time.
        assert!(crate::parse("{ }").is_err());
        // an empty `{@html}` is rejected at codegen time.
        let err = lower(&crate::parse("{@html }").unwrap()).unwrap_err();
        assert!(err.contains("empty expression"), "unexpected: {err}");
    }

    /// The core map invariant: each mapping is a verbatim copy — its generated
    /// slice equals its source slice, byte for byte. This is what lets the
    /// language server translate positions by a constant offset.
    #[test]
    fn mappings_are_verbatim() {
        let src = concat!(
            "Hi {self.name}! {@html self.body}{@render self.foot}",
            "{#if self.ok}{self.a}{:else if self.b}{self.c}{:else}{self.d}{/if}",
            "{#for (item, i) in self.items.iter().zip(0..)}{item}{i}{/for}",
            "{#for x in &self.xs}{x}{/for}",
            r#"<a href={self.url}>x</a>"#,
            r#"<Card title={2 + 8}>b</Card>"#,
            "{#snippet foo(x: u8)}{x}{/snippet}",
        );
        let t = crate::parse(src).unwrap();
        let (out, map) = lower_mapped(&t).unwrap();
        assert!(
            map.mappings.len() > 10,
            "expected many mappings, got {}",
            map.mappings.len()
        );
        for m in &map.mappings {
            assert_eq!(m.source.len(), m.generated.len());
            assert_eq!(
                &src[m.source.start..m.source.end],
                &out[m.generated.start..m.generated.end],
                "mapping {m:?} is not a verbatim copy",
            );
        }
    }

    /// A `{#for}` header emits its pattern and iterable verbatim, each mapping
    /// back to its exact source range.
    #[test]
    fn for_header_maps_pieces() {
        let src = "{#for (i, item) in self.items.iter().enumerate()}{item}{/for}";
        let t = crate::parse(src).unwrap();
        let (out, map) = lower_mapped(&t).unwrap();
        for needle in ["(i, item)", "self.items.iter().enumerate()"] {
            let m = map
                .mappings
                .iter()
                .find(|m| &src[m.source.start..m.source.end] == needle)
                .unwrap_or_else(|| panic!("no mapping for {needle:?}"));
            assert_eq!(&out[m.generated.start..m.generated.end], needle);
        }
    }

    // ------------------------------------------------------- literal layout
    //
    // These pin the *static* half: what each template's own literals look like.
    // The other half — the depth of the call site rendering the component — is
    // added at run time and tested in `damask::renderers`.

    /// The literal text a lowered template writes, in order.
    fn literals(src: &str) -> Vec<String> {
        let out = body(src);
        let mut found = Vec::new();
        let mut rest = out.as_str();
        while let Some((i, call)) = ["__damask.write_text(\"", "__damask.write_raw(\""]
            .iter()
            .filter_map(|c| rest.find(c).map(|i| (i, *c)))
            .min()
        {
            rest = &rest[i + call.len()..];
            let end = {
                let (mut j, b) = (0, rest.as_bytes());
                loop {
                    match b[j] {
                        b'\\' => j += 2,
                        b'"' => break j,
                        _ => j += 1,
                    }
                }
            };
            found.push(rest[..end].replace("\\n", "\n").replace("\\\"", "\""));
            rest = &rest[end..];
        }
        found
    }

    /// The whole point: a `{# … #}` comment leaves the newlines that surrounded
    /// it behind, and they used to reach the browser as a blank line.
    #[test]
    fn a_comment_leaves_no_blank_line_behind() {
        let out = literals("<div>\n\n  {# gone #}\n\n  <b>x</b>\n</div>").concat();
        assert!(!out.contains("\n\n"), "blank line survived: {out:?}");
        assert_eq!(out, "<div>\n  <b>x</b>\n</div>");
    }

    #[test]
    fn nesting_is_two_spaces_per_element() {
        let out = literals("<a>\n<b>\n<c>x</c>\n</b>\n</a>").concat();
        assert_eq!(out, "<a>\n  <b>\n    <c>x</c>\n  </b>\n</a>");
    }

    /// Control flow produces no tag, so it must not nest the output — the
    /// author indents inside `{#if}`, the document should not.
    #[test]
    fn control_flow_does_not_nest_the_output() {
        let src = "<a>\n  {#if c}\n    <b/>\n  {/if}\n</a>";
        assert_eq!(literals(src).concat(), "<a>\n  <b></b>\n  </a>");
        // The run before `</a>` stays at the child's depth here because whether
        // the branch rendered — and so which run it even is — is not known
        // until it runs. `close_line` settles it there.
        assert!(body(src).contains("__damask.close_line(0)"));
    }

    /// The last node of a control-flow *body* is followed by whatever comes
    /// after the `{/if}`, not by an end tag, so it must not dedent as though it
    /// were closing something — `<c/>` here has to land at the child's depth,
    /// not the element's. The final run is a different matter: it really is the
    /// element's last, and dedents.
    #[test]
    fn a_branch_does_not_dedent_its_last_line() {
        let out = literals("<a>\n  {#if c}\n    <b/>\n  {/if}\n  <c/>\n</a>").concat();
        assert_eq!(out, "<a>\n  <b></b>\n  <c></c>\n</a>");
    }

    /// A run with no newline is the author's spacing inside a line, and is
    /// content: `</b> up` must not become `</b>up`, nor gain a break.
    #[test]
    fn spacing_within_a_line_is_left_alone() {
        let out = literals("<p><b>6</b> up · <b>2</b> down</p>").concat();
        assert_eq!(out, "<p><b>6</b> up · <b>2</b> down</p>");
    }

    #[test]
    fn a_pre_keeps_its_own_whitespace() {
        let out = literals("<div>\n  <pre>\n\n   ragged\n  </pre>\n</div>").concat();
        assert!(
            out.contains("\n\n   ragged\n  "),
            "pre was reformatted: {out:?}"
        );
    }

    #[test]
    fn a_pre_brackets_the_renderer_too() {
        // A component rendered inside carries its own literals and cannot know
        // it landed in a `<pre>`, so the flag has to exist at run time as well.
        let out = body("<pre><Child/></pre>");
        assert!(out.contains("set_verbatim(true)") && out.contains("set_verbatim(false)"));
    }

    /// The shape that made the flaw visible: the last child is a conditional,
    /// so which whitespace run stands before the end tag is not known here.
    #[test]
    fn an_element_states_its_own_depth_for_its_end_tag() {
        let out = body("<a>\n  <b/>\n  {#if c}\n    <i/>\n  {/if}\n</a>");
        assert!(out.contains("__damask.close_line(0)"), "{out}");
    }

    #[test]
    fn a_nested_element_states_its_nesting() {
        let out = body("<a>\n  <b>\n    <c/>\n  </b>\n</a>");
        assert!(out.contains("__damask.close_line(1)"), "{out}");
    }

    /// Inside a verbatim element the run before the end tag is the author's,
    /// and `</pre>` sitting where they put it is the point.
    #[test]
    fn a_verbatim_element_does_not_close_its_line() {
        let out = body("<pre>\n  x\n</pre>");
        assert!(!out.contains("close_line"), "{out}");
    }

    #[test]
    fn a_child_is_bracketed_with_the_depth_of_its_call_site() {
        let out = body("<a>\n  <b>\n    <Card/>\n  </b>\n</a>");
        assert!(out.contains("push_indent(2)"), "{out}");
        assert!(out.contains("pop_indent(2)"), "{out}");
    }

    /// Markup at the root of a template needs no adjustment, and emitting the
    /// calls anyway would cost every page a pair of no-ops per component.
    #[test]
    fn a_child_at_the_root_is_not_bracketed() {
        let out = body("<Card/>");
        assert!(!out.contains("push_indent"), "{out}");
    }

    /// Slot content is written in the caller and laid out from the caller's
    /// root, because where it lands is the callee's business. The depth is
    /// applied by `Slots::render`, which is the only place that knows whether
    /// the fill or the fallback was taken.
    #[test]
    fn a_slot_fill_is_laid_out_from_its_own_root() {
        let out = literals("<a>\n  <Card>\n    <b>\n      <c/>\n    </b>\n  </Card>\n</a>");
        let all = out.concat();
        assert!(
            all.contains("<b>\n  <c></c>\n</b>"),
            "fill must start at column 0: {all:?}"
        );
    }

    #[test]
    fn a_slot_declares_its_depth_to_the_renderer() {
        let out = body("<div>\n  <p>\n    <slot/>\n  </p>\n</div>");
        assert!(
            out.contains("__damask_slots.render(\"\", &mut *__damask, 2,"),
            "{out}"
        );
    }

    // Async lowering: a template with `.await` anywhere switches every render
    // call it makes to the async, awaited form. A template with none of that
    // stays on today's plain sync path — asserted throughout this file above.

    #[test]
    fn a_plain_expr_tag_awaits_inline_without_changing_call_forms() {
        let out = body("{self.fetch().await}");
        assert!(out.contains("::damask::splice(&(self.fetch().await), &mut *__damask)"));
    }

    #[test]
    fn render_tag_awaits_in_an_async_template() {
        let out = body("{self.other().await}{@render self.child}");
        assert!(
            out.contains(
                "::damask::AsyncRender::render_into_async(&(self.child), &mut *__damask).await;"
            ),
            "{out}"
        );
        assert!(!out.contains("::damask::Render::render_into("), "{out}");
    }

    #[test]
    fn render_tag_stays_sync_with_no_await_anywhere() {
        let out = body("{@render self.child}");
        assert!(out.contains("::damask::Render::render_into(&(self.child), &mut *__damask);"));
    }

    #[test]
    fn component_element_awaits_in_an_async_template() {
        let out = body("{self.other().await}<Card/>");
        assert!(
            out.contains(
                "::damask::AsyncRender::render_into_async(&(Card::__damask_props()
.__damask_build()), &mut *__damask).await;"
            ),
            "{out}"
        );
    }

    #[test]
    fn component_element_with_slots_awaits_in_an_async_template() {
        let out = body("{self.other().await}<Card><p>hi</p></Card>");
        assert!(
            out.contains("::damask::AsyncRender::render_slots_async("),
            "{out}"
        );
        assert!(out.contains("])).await;"), "{out}");
        // The fill itself stays a plain sync `Fragment`, never an awaited call.
        assert!(out.contains("::damask::fragment(|__damask"));
        assert!(!out.contains("Box::pin"), "{out}");
    }

    #[test]
    fn slot_placeholder_awaits_in_an_async_template() {
        let out = body("{self.other().await}<div><slot>fallback</slot></div>");
        assert!(
            out.contains("__damask_slots.render_async(\"\", &mut *__damask, 1,"),
            "{out}"
        );
        assert!(out.contains("::std::boxed::Box::pin(async move"), "{out}");
        assert!(out.contains(".await;"), "{out}");
    }

    #[test]
    fn a_snippet_that_itself_awaits_becomes_an_async_fragment() {
        // No params: nothing here is moved into the inner `async move` block
        // except `self`'s own reference (`Copy`), so this is safe to call
        // more than once, as `Render`/`AsyncRender` require.
        let out = body("{#snippet item()}{self.other().await}{/snippet}{@render item()}");
        assert!(out.contains("::damask::fragment_async("), "{out}");
        assert!(out.contains("::std::boxed::Box::pin(async move"), "{out}");
    }

    /// A snippet parameter carries no type, and needs none: `splice` is a
    /// generic function, so the value's type arrives from wherever the snippet
    /// is rendered and picks its own `Value` impl there.
    #[test]
    fn an_untyped_snippet_parameter_is_spliced_like_anything_else() {
        let out = body("{#snippet item(x)}{x}{/snippet}{@render item(1)}");
        assert!(
            out.contains("::damask::splice(&(x), &mut *__damask)"),
            "{out}"
        );
    }

    #[test]
    fn snippet_stays_a_plain_fragment_with_no_await_anywhere() {
        let out = body("{#snippet item(x: u8)}{x}{/snippet}{@render item(1)}");
        assert!(out.contains("::damask::fragment(move"), "{out}");
        assert!(!out.contains("fragment_async"), "{out}");
    }

    /// A parameterized snippet that does *not* itself await stays a plain
    /// `Fragment` even inside an otherwise-async template: `{@render}`-ing it
    /// still goes through the awaited `AsyncRender` call (every `Render` gets
    /// that for free), but the snippet's own closure needs no future of its
    /// own — which is what keeps a non-`Copy` parameter movable into it on
    /// every call.
    #[test]
    fn a_parameterized_snippet_with_no_await_of_its_own_stays_sync_in_an_async_template() {
        let out = body(
            "{self.other().await}{#snippet item(x: String)}{x}{/snippet}{@render item(self.other().await)}",
        );
        assert!(out.contains("::damask::fragment(move"), "{out}");
        assert!(!out.contains("fragment_async"), "{out}");
        assert!(
            out.contains("::damask::AsyncRender::render_into_async(&(item(self.other().await)), &mut *__damask).await;"),
            "{out}"
        );
    }

    #[test]
    fn a_parameterized_snippet_that_awaits_is_a_clear_error() {
        let err = lower(
            &crate::parse(
                "{#snippet item(x: String)}{compute(x).await}{/snippet}{@render item(1)}",
            )
            .unwrap(),
        )
        .unwrap_err();
        assert!(err.contains("item"), "{err}");
        assert!(err.contains("parameters"), "{err}");
    }

    /// The marker that makes an async-only child usable: nothing about
    /// `<Slow/>` says whether it renders asynchronously, because it is a type
    /// and this pass sees markup.
    #[test]
    fn an_awaited_component_takes_the_async_path() {
        let out = body("<Slow await/>");
        assert!(out.contains("render_into_async"), "{out}");
        assert!(out.contains(".await"), "{out}");
        // And it is not passed on as a prop, which would not compile: `await`
        // is a keyword.
        assert!(!out.contains(".await("), "{out}");
    }

    #[test]
    fn an_awaited_component_with_slots_takes_the_async_path() {
        let out = body("<Card await><p>hi</p></Card>");
        assert!(out.contains("render_slots_async"), "{out}");
    }

    /// The marker on an HTML element is a mistake worth naming: silently
    /// writing `await` into the markup would leave the component above it
    /// failing to compile with nothing to point at.
    #[test]
    fn the_await_marker_on_an_html_element_is_a_clear_error() {
        let err = lower(&crate::parse("<div await></div>").unwrap()).unwrap_err();
        assert!(err.contains("component"), "{err}");
        assert!(err.contains("div"), "{err}");
    }

    /// Markup between a component's tags may suspend, which is what lets an
    /// awaiting component be the child of a wrapper — `<Card><Slow/></Card>`,
    /// and anything built on that shape, a cached fragment included.
    #[test]
    fn a_default_slot_fill_may_await() {
        let out = body("<Card>{self.fetch().await}</Card>");
        assert!(
            out.contains("Slot::new_async(::damask::DEFAULT_SLOT"),
            "{out}"
        );
        assert!(out.contains("fragment_async"), "{out}");
        assert!(out.contains("Box::pin(async move"), "{out}");
        // The fill is awaited where it is written, so the call taking it has to
        // be the async one.
        assert!(out.contains("render_slots_async"), "{out}");
    }

    #[test]
    fn a_named_slot_fill_may_await() {
        let out = body(r#"<Card><p slot="foot">{self.fetch().await}</p></Card>"#);
        assert!(out.contains("Slot::new_async(\"foot\""), "{out}");
        assert!(out.contains("fragment_async"), "{out}");
    }

    /// Only the fill that genuinely suspends pays for the boxed future: one
    /// awaiting fill beside a plain one leaves the plain one a `Fragment`.
    #[test]
    fn a_fill_that_does_not_await_stays_a_plain_fragment() {
        let out = body(r#"<Card><p slot="foot">{self.fetch().await}</p>plain</Card>"#);
        assert!(out.contains("Slot::new(::damask::DEFAULT_SLOT"), "{out}");
        assert!(out.contains("Slot::new_async(\"foot\""), "{out}");
    }

    /// An await elsewhere in the template does not make a fill async: what
    /// decides is the fill's own body, so a nested component inside one that
    /// suspends nowhere still renders through the plain sync call and costs no
    /// boxed future.
    #[test]
    fn a_slot_fill_stays_sync_even_when_the_template_is_otherwise_async() {
        let out = body("{self.other().await}<Card><Inner/></Card>");
        assert!(
            out.contains(
                "::damask::Render::render_into(&(Inner::__damask_props()
.__damask_build()), &mut *__damask);"
            ),
            "{out}"
        );
    }
}
