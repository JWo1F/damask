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
    Attr, AttrPart, AttrValue, ClassTerm, EachNode, Element, ElementKind, IfNode, Node,
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
/// Entries are pushed in generated order (which, apart from `{#each … as p, i}`
/// swapping pattern and index, is also source order).
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
}

impl Emit {
    fn new() -> Self {
        Emit {
            out: String::new(),
            map: SourceMap::default(),
            at_line_start: false,
        }
    }

    /// Append scaffolding that does not correspond to any source range.
    fn raw(&mut self, s: &str) {
        self.out.push_str(s);
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

/// Lower a template to the body of `render_into`, as a string of Rust source.
///
/// The result is a single brace-delimited block; the caller parses it once so
/// control-flow tags whose braces span multiple `{ }` block tags balance as one
/// Rust block.
pub fn lower(template: &Template) -> Result<String, String> {
    lower_mapped(template).map(|(body, _)| body)
}

/// Like [`lower`], but also returns the [`SourceMap`] tying generated ranges
/// back to the template.
pub fn lower_mapped(template: &Template) -> Result<(String, SourceMap), String> {
    let mut e = Emit::new();
    // Bring `Component`/`Render` into scope (unnamed) so `child.render()` and
    // `{@render …}`-style calls resolve without the author importing the traits.
    e.raw("{\n#[allow(unused_imports)] use ::damask::{Component as _, Render as _};\n");
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

    /// The layout for a control-flow body — `{#if}`, `{#each}`. These are not
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
            e.raw("__damask.write_display_raw(::damask::as_display(&(");
            e.frag(code);
            e.raw(")));\n");
            e.at_line_start = false;
        }
        // A snippet or fragment is laid out from its own root, like a
        // component, so the depth of the site rendering it is added here.
        Node::Render(code) => {
            require_expr(code.as_str(), "{@render … }")?;
            indented(layout.depth, e, |e| {
                e.raw("::damask::Render::render_into(&(");
                e.frag(code);
                e.raw("), &mut *__damask);\n");
            });
            e.at_line_start = false;
        }
        Node::If(if_node) => emit_if(if_node, layout.same(), e)?,
        Node::Each(each) => emit_each(each, layout.same(), e)?,
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
/// ends in `;`, otherwise print its value (escaped).
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
        e.raw("__damask.write_escaped(::damask::as_display(&({ ");
        e.frag(code);
        e.raw(" })));\n");
    } else {
        // A plain expression: borrow it directly (no block) so field access
        // like `self.name` borrows rather than moves out of `&self`.
        e.raw("__damask.write_escaped(::damask::as_display(&(");
        e.frag(code);
        e.raw(")));\n");
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

/// `{#each E as p}` → `for p in E {`, and `{#each E as p, i}` →
/// `for (i, p) in (E).into_iter().enumerate() {`.
fn emit_each(each: &EachNode, layout: Layout, e: &mut Emit) -> Result<(), String> {
    // An empty iterator runs the body no times, so the state after the loop is
    // known only where the body agrees with the state before it.
    let before = e.at_line_start;
    let (expr, binding) = (each.expr.as_str().trim(), each.binding.as_str().trim());
    if expr.is_empty() || binding.is_empty() {
        return Err("malformed `{#each}`".into());
    }
    // A trailing `, ident` is the index form; anything else (e.g. a tuple
    // pattern `(a, b)`) is treated as the whole pattern.
    if let Some(comma) = binding.rfind(',') {
        let pat = sub_fragment(&each.binding, &binding[..comma]);
        let idx = sub_fragment(&each.binding, &binding[comma + 1..]);
        let is_ident = !idx.text.is_empty()
            && idx.text.chars().all(|c| c.is_alphanumeric() || c == '_')
            && idx
                .text
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_');
        if is_ident && !pat.text.is_empty() {
            e.raw("for (");
            e.frag(&idx);
            e.raw(", ");
            e.frag(&pat);
            e.raw(") in (");
            e.frag(&each.expr);
            e.raw(").into_iter().enumerate() {\n");
            emit_nodes(&each.body, layout, e)?;
            e.raw("}\n");
            e.at_line_start &= before;
            return Ok(());
        }
    }
    e.raw("for ");
    e.frag(&each.binding);
    e.raw(" in ");
    e.frag(&each.expr);
    e.raw(" {\n");
    emit_nodes(&each.body, layout, e)?;
    e.raw("}\n");
    e.at_line_start &= before;
    Ok(())
}

/// Build a [`Spanned`] for `piece`, a trimmed sub-slice of `whole.text`, with a
/// span offset into the source to match. `piece` must be a substring of
/// `whole.text` (typically the result of slicing then trimming it).
fn sub_fragment(whole: &Spanned, piece: &str) -> Spanned {
    // Locate `piece` within `whole.text` by its trimmed bounds. Both the slice
    // passed in and `whole.text` share the same buffer offsets because `whole`
    // is itself a verbatim slice of the source (parser invariant).
    let lead = piece.len() - piece.trim_start().len();
    let trimmed = piece.trim();
    let offset = piece.as_ptr() as usize - whole.text.as_ptr() as usize + lead;
    let start = whole.span.start + offset;
    Spanned::new(trimmed, Span::new(start, start + trimmed.len()))
}

fn emit_snippet(snippet: &SnippetNode, e: &mut Emit) -> Result<(), String> {
    if snippet.name.as_str().is_empty() {
        return Err("`{#snippet}` needs a name".into());
    }
    if snippet.params.as_str().is_empty() {
        e.raw("let ");
        e.frag(&snippet.name);
        e.raw(" = ::damask::fragment(|__damask: &mut dyn ::damask::Renderer| {\n");
    } else {
        e.raw("let ");
        e.frag(&snippet.name);
        e.raw(" = |");
        e.frag(&snippet.params);
        e.raw("| ::damask::fragment(move |__damask: &mut dyn ::damask::Renderer| {\n");
    }
    e.at_line_start = false;
    emit_nodes(&snippet.body, Layout::ROOT, e)?;
    e.raw("});\n");
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
            AttrValue::Spread(code) => {
                require_expr(code.as_str(), "{...} attribute spread")?;
                flush_raw(&mut raw, e);
                e.raw("::damask::AttrSpread::write_attrs(&(");
                e.frag(code);
                e.raw("), &mut *__damask);\n");
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
                            e.raw("__damask.write_escaped(::damask::as_display(&(");
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
                e.raw("::damask::Attr::write_attr(&(");
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

/// `<slot/>` / `<slot name="x">fallback</slot>` in a component template — render
/// what the caller passed for that slot, or the `<slot>`'s own body if unfilled.
fn emit_slot_placeholder(el: &Element, layout: Layout, e: &mut Emit) -> Result<(), String> {
    let name = slot_name(el)?;
    // The fill was written in the caller and laid out from *its* root, because
    // where it lands is this template's business, not the caller's — so the
    // slot's depth is added to it here. The fallback below is this template's
    // own markup and already carries that depth, so the two cannot share one
    // bracket: `Slots::render` applies the depth to whichever it takes.
    e.raw(&format!(
        "__damask_slots.render({name:?}, &mut *__damask, {}, |__damask: &mut dyn ::damask::Renderer| {{\n",
        layout.depth
    ));
    e.at_line_start = false;
    emit_nodes(&el.children, layout.same(), e)?;
    e.raw("});\n");
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
    e.raw("{\nlet mut __damask_class = ::damask::ClassList::new();\n");

    match value {
        None => {}
        Some(AttrValue::Classes(terms)) => {
            for term in terms {
                match term {
                    ClassTerm::Nothing => {}
                    ClassTerm::Expr(code) => {
                        require_expr(code.as_str(), "class list entry")?;
                        e.raw("::damask::ClassItem::add_to(&(");
                        e.frag(code);
                        e.raw("), &mut __damask_class);\n");
                    }
                    ClassTerm::Cond { name, when } => {
                        require_expr(when.as_str(), "class condition")?;
                        // Spliced bare, as `{#if}` does: parenthesising warns
                        // `unused_parens` in the user's crate, not in ours.
                        e.raw("if ");
                        e.frag(when);
                        e.raw(" { ::damask::ClassItem::add_to(&(");
                        e.frag(name);
                        e.raw("), &mut __damask_class); }\n");
                    }
                }
            }
        }
        Some(AttrValue::Literal(parts)) => {
            e.raw("::damask::ClassItem::add_to(&(");
            emit_literal_string(parts, e)?;
            e.raw("), &mut __damask_class);\n");
        }
        Some(AttrValue::Expr(code)) => {
            require_expr(code.as_str(), "class")?;
            e.raw("::damask::ClassItem::add_to(&(");
            e.frag(code);
            e.raw("), &mut __damask_class);\n");
        }
        Some(AttrValue::Boolean) => return Err("`class` needs a value".into()),
        // A spread carries its own names, so it never reaches here as `class`.
        Some(AttrValue::Spread(_)) => unreachable!("a spread has no attribute name"),
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

/// `<Comp attr={e}>…</Comp>` — build `Comp { attr: e }` and render it with the
/// element's content as its slot fills.
fn emit_component_element(el: &Element, layout: Layout, e: &mut Emit) -> Result<(), String> {
    // Partition children into named-slot fills and default-slot content. A
    // *named* `<slot>` directly inside the element names the slot it fills; a
    // bare `<slot/>` names nothing, so it stays ordinary default-slot content —
    // a placeholder that forwards this component's own default slot, and one
    // that can sit alongside other markup in the same fill.
    let mut default: Vec<&Node> = Vec::new();
    let mut named: Vec<(String, &[Node])> = Vec::new();
    for child in &el.children {
        let fill = match child {
            Node::Element(slot) if slot.kind == ElementKind::Slot => {
                let name = slot_name(slot)?;
                (!name.is_empty()).then_some((name, slot.children.as_slice()))
            }
            _ => None,
        };
        match fill {
            Some((name, body)) => {
                if named.iter().any(|(seen, _)| *seen == name) {
                    return Err(format!("slot `{name}` is filled twice"));
                }
                named.push((name, body));
            }
            None => default.push(child),
        }
    }

    // Default slot: filled only when there is real (non-whitespace) content.
    let has_default = default
        .iter()
        .any(|n| !matches!(n, Node::Text(t) if t.as_str().trim().is_empty()));

    // The fills borrow temporaries that live to the end of this statement, so
    // slot content stays on the stack and can borrow the enclosing scope.
    let method = if has_default || !named.is_empty() {
        "render_slots"
    } else {
        "render_into"
    };
    // The component's markup is laid out from its own root, so the depth of
    // this call site is what places it. The bracket spans the whole statement
    // rather than just the call, because the slot fills below are closures the
    // callee invokes while it runs — they need the same depth, plus the one the
    // callee's `<slot>` adds.
    if layout.depth > 0 {
        e.raw(&format!("__damask.push_indent({});\n", layout.depth));
    }
    e.raw(&format!("::damask::Render::{method}(&("));
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
                e.raw(".");
                e.frag(&attr.name);
                e.raw("(");
                if !interpolating {
                    e.raw("::damask::props::literal(");
                }
                emit_literal_string(parts, e)?;
                e.raw(if interpolating { ".into())\n" } else { "))\n" });
            }
            // `.into()` for the same reason a quoted value has one: the bare
            // form is how `flag` and `flag={true}` are written, and it should
            // reach an `Option<bool>` prop as readily as a `bool` one.
            AttrValue::Boolean => {
                e.raw(".");
                e.frag(&attr.name);
                e.raw("(true.into())\n");
            }
            // A class list assembles markup, and a component prop is a value.
            // `class={…}` with an ordinary expression is the way to pass one.
            AttrValue::Classes(_) => {
                return Err(format!(
                    "`{}` is a component prop, so it cannot take a class list",
                    attr.name.as_str()
                ));
            }
            // Spreading fills in attributes, and a component has fields. There
            // is no field name to give the value, so there is nothing to build.
            AttrValue::Spread(_) => {
                return Err(
                    "`{...}` spreads attributes onto an HTML element; a component takes named props"
                        .into(),
                );
            }
        }
    }

    e.raw(".__damask_build()), &mut *__damask");

    if method == "render_into" {
        e.raw(");\n");
        e.at_line_start = false;
        if layout.depth > 0 {
            e.raw(&format!("__damask.pop_indent({});\n", layout.depth));
        }
        return Ok(());
    }

    e.raw(", ::damask::Slots::new(&[\n");
    if has_default {
        e.raw("::damask::Slot::new(::damask::DEFAULT_SLOT, &::damask::fragment(|__damask: &mut dyn ::damask::Renderer| {\n");
        e.at_line_start = false;
        for (i, n) in default.iter().enumerate() {
            emit_node(n, Layout::ROOT, i + 1 == default.len(), e)?;
        }
        e.raw("})),\n");
    }
    for (name, body) in &named {
        e.raw(&format!(
            "::damask::Slot::new({name:?}, &::damask::fragment(|__damask: &mut dyn ::damask::Renderer| {{\n"
        ));
        e.at_line_start = false;
        emit_nodes(body, Layout::ROOT, e)?;
        e.raw("})),\n");
    }
    e.raw("]));\n");
    e.at_line_start = false;
    if layout.depth > 0 {
        e.raw(&format!("__damask.pop_indent({});\n", layout.depth));
    }
    Ok(())
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
        assert!(b.contains("__damask.write_escaped(::damask::as_display(&(self.name)))"));
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
        assert!(body("{2 + 3; 10}").contains("write_escaped(::damask::as_display(&({ 2 + 3; 10 })))"));
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
    fn if_and_each() {
        let b = body("{#if self.a}x{:else}y{/if}");
        assert!(b.contains("if self.a {"));
        assert!(b.contains("} else {"));
        assert!(
            body("{#each &self.items as item}{item}{/each}").contains("for item in &self.items {")
        );
        assert!(
            body("{#each &self.items as item, i}{i}{/each}")
                .contains("for (i, item) in (&self.items).into_iter().enumerate() {")
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
    #[test]
    fn interpolating_value_on_a_component_prop_is_owned() {
        let b = body(r#"<Comp class="a {self.x} b"/>"#);
        assert!(
            b.contains(r#".class(::std::format!("a {} b", self.x).into())"#),
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
    fn attribute_spread() {
        let b = body(r#"<div {...self.extra}></div>"#);
        assert!(b.contains("::damask::AttrSpread::write_attrs(&(self.extra), &mut *__damask);"));
        // A component takes named props, so there is nothing to spread onto.
        assert!(lower(&crate::parse(r#"<Comp {...self.extra}/>"#).unwrap()).is_err());
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
        let b = body(r#"<Card title={2 + 8} tag="h1">body<slot name="foot">f</slot></Card>"#);
        assert!(b.contains("::damask::Render::render_slots(&(Card::__damask_props()"));
        assert!(b.contains(".title((2 + 8))"));
        assert!(b.contains(r#".tag(::damask::props::literal("h1"))"#));
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
            body(r#"<slot name="foot"/>"#).contains(r#"__damask_slots.render("foot", &mut *__damask"#)
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
    fn bare_slot_in_a_component_forwards_the_default_slot() {
        // Not a fill directive: it lowers as content *inside* the default fill,
        // so it forwards this component's own default slot.
        let b = body("<Card><slot/></Card>");
        let fill = b.find("::damask::Slot::new(::damask::DEFAULT_SLOT").unwrap();
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

    #[test]
    fn filling_a_slot_twice_is_an_error() {
        let src = r#"<Card><slot name="a">1</slot><slot name="a">2</slot></Card>"#;
        let err = lower(&crate::parse(src).unwrap()).unwrap_err();
        assert!(err.contains("filled twice"), "unexpected: {err}");
    }

    #[test]
    fn empty_slot_name_is_an_error() {
        let err = lower(&crate::parse(r#"<slot name=""/>"#).unwrap()).unwrap_err();
        assert!(err.contains("must not be empty"), "unexpected: {err}");
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
            "{#each &self.items as item, i}{item}{i}{/each}",
            "{#each &self.xs as x}{x}{/each}",
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

    /// `{#each E as pat, i}` swaps pattern and index in the output; both, and the
    /// iterable, must still map back to their exact source ranges.
    #[test]
    fn each_index_form_maps_reordered_pieces() {
        let src = "{#each &self.items as item, i}{item}{/each}";
        let t = crate::parse(src).unwrap();
        let (out, map) = lower_mapped(&t).unwrap();
        for needle in ["item", "i", "&self.items"] {
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
}
