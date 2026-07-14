//! Lower a parsed [`Template`] to the body of a `render_into` method, as a
//! string of Rust source.
//!
//! This is the single source of truth for template → Rust code generation,
//! shared by two consumers so they can never disagree about what a template
//! means:
//!
//! - the `Component` derive in `rsc-macros`, which splices this body into an
//!   `impl` at compile time, and
//! - the language server, which wraps the same body in a virtual `impl` block
//!   and hands it to `rust-analyzer`.
//!
//! It emits *only* the block body (`{ … }`); the caller supplies the
//! surrounding `fn render_into(&self, __rsc: &mut dyn Renderer)` signature.
//!
//! Alongside the text, [`lower_mapped`] returns a [`SourceMap`]: for every Rust
//! fragment copied verbatim from the template, a correspondence between its
//! `.rsc` byte range and the byte range it occupies in the generated body.
//! Because each such copy is byte-identical, the two ranges are always the same
//! length, so the language server can translate a position from one side to the
//! other by a constant offset.

use crate::{
    AttrValue, EachNode, Element, ElementKind, IfNode, Node, SnippetNode, Span, Spanned, Template,
    is_void_element,
};

/// A verbatim correspondence between a `.rsc` source range and the generated
/// Rust range it was copied into. Both ranges cover byte-identical text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mapping {
    /// Byte range in the `.rsc` template source.
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
}

impl Emit {
    fn new() -> Self {
        Emit {
            out: String::new(),
            map: SourceMap::default(),
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
    e.raw("{\n#[allow(unused_imports)] use ::rsc::{Component as _, Render as _};\n");
    emit_nodes(&template.nodes, &mut e)?;
    e.raw("}\n");
    Ok((e.out, e.map))
}

fn emit_nodes(nodes: &[Node], e: &mut Emit) -> Result<(), String> {
    for node in nodes {
        emit_node(node, e)?;
    }
    Ok(())
}

fn emit_node(node: &Node, e: &mut Emit) -> Result<(), String> {
    match node {
        // Text becomes an escaped string literal — not a verbatim copy, so it is
        // not mapped here; text positions belong to the HTML virtual document.
        Node::Text(text) => e.raw(&format!("__rsc.write_raw({:?});\n", text.as_str())),
        Node::Expr(code) => emit_expr(code, e),
        Node::Html(code) => {
            require_expr(code.as_str(), "{@html … }")?;
            e.raw("__rsc.write_display_raw(::rsc::as_display(&(");
            e.frag(code);
            e.raw(")));\n");
        }
        Node::Render(code) => {
            require_expr(code.as_str(), "{@render … }")?;
            e.raw("::rsc::Render::render_into(&(");
            e.frag(code);
            e.raw("), &mut *__rsc);\n");
        }
        Node::If(if_node) => emit_if(if_node, e)?,
        Node::Each(each) => emit_each(each, e)?,
        Node::Snippet(snippet) => emit_snippet(snippet, e)?,
        Node::Element(element) => emit_element(element, e)?,
    }
    Ok(())
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
        e.raw("__rsc.write_escaped(::rsc::as_display(&({ ");
        e.frag(code);
        e.raw(" })));\n");
    } else {
        // A plain expression: borrow it directly (no block) so field access
        // like `self.name` borrows rather than moves out of `&self`.
        e.raw("__rsc.write_escaped(::rsc::as_display(&(");
        e.frag(code);
        e.raw(")));\n");
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

fn emit_if(if_node: &IfNode, e: &mut Emit) -> Result<(), String> {
    for (i, (cond, body)) in if_node.branches.iter().enumerate() {
        require_expr(cond.as_str(), "{#if … }")?;
        if i == 0 {
            e.raw("if ");
        } else {
            e.raw("} else if ");
        }
        e.frag(cond);
        e.raw(" {\n");
        emit_nodes(body, e)?;
    }
    if let Some(otherwise) = &if_node.otherwise {
        e.raw("} else {\n");
        emit_nodes(otherwise, e)?;
    }
    e.raw("}\n");
    Ok(())
}

/// `{#each E as p}` → `for p in E {`, and `{#each E as p, i}` →
/// `for (i, p) in (E).into_iter().enumerate() {`.
fn emit_each(each: &EachNode, e: &mut Emit) -> Result<(), String> {
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
            emit_nodes(&each.body, e)?;
            e.raw("}\n");
            return Ok(());
        }
    }
    e.raw("for ");
    e.frag(&each.binding);
    e.raw(" in ");
    e.frag(&each.expr);
    e.raw(" {\n");
    emit_nodes(&each.body, e)?;
    e.raw("}\n");
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
        e.raw(" = ::rsc::fragment(|__rsc: &mut dyn ::rsc::Renderer| {\n");
    } else {
        e.raw("let ");
        e.frag(&snippet.name);
        e.raw(" = |");
        e.frag(&snippet.params);
        e.raw("| ::rsc::fragment(move |__rsc: &mut dyn ::rsc::Renderer| {\n");
    }
    emit_nodes(&snippet.body, e)?;
    e.raw("});\n");
    Ok(())
}

fn emit_element(el: &Element, e: &mut Emit) -> Result<(), String> {
    match el.kind {
        ElementKind::Html => emit_html_element(el, e),
        ElementKind::Component => emit_component_element(el, e),
        ElementKind::Slot => emit_slot_placeholder(el, e),
    }
}

/// Append `__rsc.write_raw("…")` for `raw`, if non-empty, and clear it.
fn flush_raw(raw: &mut String, e: &mut Emit) {
    if !raw.is_empty() {
        e.raw(&format!("__rsc.write_raw({raw:?});\n"));
        raw.clear();
    }
}

fn emit_html_element(el: &Element, e: &mut Emit) -> Result<(), String> {
    let mut raw = String::new();
    raw.push('<');
    raw.push_str(el.tag.as_str());

    for attr in &el.attrs {
        match &attr.value {
            AttrValue::Boolean => {
                raw.push(' ');
                raw.push_str(attr.name.as_str());
            }
            AttrValue::Literal(v) => {
                raw.push(' ');
                raw.push_str(attr.name.as_str());
                raw.push_str("=\"");
                raw.push_str(v.as_str());
                raw.push('"');
            }
            AttrValue::Expr(code) => {
                require_expr(code.as_str(), "attribute value")?;
                raw.push(' ');
                raw.push_str(attr.name.as_str());
                raw.push_str("=\"");
                flush_raw(&mut raw, e);
                e.raw("__rsc.write_escaped(::rsc::as_display(&(");
                e.frag(code);
                e.raw(")));\n");
                raw.push('"');
            }
        }
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

    // A scope block so `{use}` (and bindings) are scoped to this element.
    e.raw("{\n");
    emit_nodes(&el.children, e)?;
    e.raw("}\n");

    e.raw(&format!(
        "__rsc.write_raw({:?});\n",
        format!("</{}>", el.tag.as_str())
    ));
    Ok(())
}

/// `<slot/>` / `<slot name="x">fallback</slot>` in a component template — render
/// what the caller passed for that slot, or the `<slot>`'s own body if unfilled.
fn emit_slot_placeholder(el: &Element, e: &mut Emit) -> Result<(), String> {
    let name = slot_name(el)?;
    e.raw(&format!(
        "__rsc_slots.render({name:?}, &mut *__rsc, |__rsc: &mut dyn ::rsc::Renderer| {{\n"
    ));
    emit_nodes(&el.children, e)?;
    e.raw("});\n");
    Ok(())
}

/// A `<slot>`'s name: the `name="…"` attribute, or [`DEFAULT_SLOT`] (empty) for
/// the unnamed default slot.
///
/// [`DEFAULT_SLOT`]: https://docs.rs/rsc/latest/rsc/constant.DEFAULT_SLOT.html
fn slot_name(el: &Element) -> Result<String, String> {
    match el.attrs.iter().find(|a| a.name == "name") {
        None => Ok(String::new()),
        Some(attr) => match &attr.value {
            AttrValue::Literal(name) if name.text.is_empty() => {
                Err("`<slot name>` must not be empty; write `<slot/>` for the default slot".into())
            }
            AttrValue::Literal(name) => Ok(name.text.clone()),
            _ => Err("`<slot name>` must be a string literal".into()),
        },
    }
}

/// `<Comp attr={e}>…</Comp>` — build `Comp { attr: e }` and render it with the
/// element's content as its slot fills.
fn emit_component_element(el: &Element, e: &mut Emit) -> Result<(), String> {
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
    e.raw(&format!("::rsc::Render::{method}(&("));
    // The tag name and each attribute name are spliced as *mapped* fragments:
    // they land on the struct name and its field initialisers, so the language
    // server can answer hover and go-to-definition over `<Comp attr=…>` itself,
    // not just the Rust inside the attribute values.
    e.frag(&el.tag);
    e.raw(" {\n");

    for attr in &el.attrs {
        match &attr.value {
            AttrValue::Expr(code) => {
                require_expr(code.as_str(), "attribute value")?;
                e.frag(&attr.name);
                e.raw(": (");
                e.frag(code);
                e.raw("),\n");
            }
            AttrValue::Literal(v) => {
                e.frag(&attr.name);
                e.raw(&format!(": {:?}.into(),\n", v.text));
            }
            AttrValue::Boolean => {
                e.frag(&attr.name);
                e.raw(": true,\n");
            }
        }
    }

    e.raw("}), &mut *__rsc");

    if method == "render_into" {
        e.raw(");\n");
        return Ok(());
    }

    e.raw(", ::rsc::Slots::new(&[\n");
    if has_default {
        e.raw("::rsc::Slot::new(::rsc::DEFAULT_SLOT, &::rsc::fragment(|__rsc: &mut dyn ::rsc::Renderer| {\n");
        for n in &default {
            emit_node(n, e)?;
        }
        e.raw("})),\n");
    }
    for (name, body) in &named {
        e.raw(&format!(
            "::rsc::Slot::new({name:?}, &::rsc::fragment(|__rsc: &mut dyn ::rsc::Renderer| {{\n"
        ));
        emit_nodes(body, e)?;
        e.raw("})),\n");
    }
    e.raw("]));\n");
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
        assert!(b.contains(r#"__rsc.write_raw("Hi ")"#));
        assert!(b.contains("__rsc.write_escaped(::rsc::as_display(&(self.name)))"));
        assert!(b.contains(r#"__rsc.write_raw("!")"#));
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
        assert!(body("{2 + 3; 10}").contains("write_escaped(::rsc::as_display(&({ 2 + 3; 10 })))"));
    }

    #[test]
    fn directives_and_use() {
        assert!(
            body("{@html self.body}")
                .contains("write_display_raw(::rsc::as_display(&(self.body)))")
        );
        assert!(
            body("{@render self.footer}").contains("::rsc::Render::render_into(&(self.footer)")
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
        assert!(b.contains("__rsc.write_escaped(::rsc::as_display(&(self.id)))"));
        // element content is a scope block containing the use
        assert!(b.contains("use crate::X;"));
        assert!(b.contains(r#"write_raw("</div>")"#));
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
        assert!(b.contains("::rsc::Render::render_slots(&(Card {"));
        assert!(b.contains("title: (2 + 8),"));
        assert!(b.contains(r#"tag: "h1".into(),"#));
        assert!(b.contains("::rsc::Slot::new(::rsc::DEFAULT_SLOT, &::rsc::fragment("));
        assert!(b.contains(r#"::rsc::Slot::new("foot", &::rsc::fragment("#));
    }

    #[test]
    fn component_element_without_content_skips_slots() {
        // Nothing to fill: no slot slice is built, and the plain render path is
        // used — same call `{@render …}` emits.
        let b = body(r#"<Card title="x"/>"#);
        assert!(b.contains("::rsc::Render::render_into(&(Card {"));
        assert!(!b.contains("::rsc::Slots::new"));
    }

    #[test]
    fn slot_placeholder_resolves_against_the_caller_slots() {
        assert!(body("<slot/>").contains(r#"__rsc_slots.render("", &mut *__rsc"#));
        assert!(
            body(r#"<slot name="foot"/>"#).contains(r#"__rsc_slots.render("foot", &mut *__rsc"#)
        );
    }

    #[test]
    fn slot_fallback_body_is_the_unfilled_branch() {
        let b = body(r#"<slot name="foot">fallback</slot>"#);
        let call = b.find(r#"__rsc_slots.render("foot""#).unwrap();
        let fallback = b.find(r#"write_raw("fallback")"#).unwrap();
        assert!(call < fallback, "fallback body belongs to the closure");
    }

    #[test]
    fn bare_slot_in_a_component_forwards_the_default_slot() {
        // Not a fill directive: it lowers as content *inside* the default fill,
        // so it forwards this component's own default slot.
        let b = body("<Card><slot/></Card>");
        let fill = b.find("::rsc::Slot::new(::rsc::DEFAULT_SLOT").unwrap();
        let forward = b.find(r#"__rsc_slots.render("""#).unwrap();
        assert!(
            fill < forward,
            "forwarding placeholder sits inside the fill"
        );
    }

    #[test]
    fn a_forwarded_slot_mixes_with_other_content() {
        let b = body("<Card>before<slot/>after</Card>");
        let before = b.find(r#"write_raw("before")"#).unwrap();
        let forward = b.find(r#"__rsc_slots.render("""#).unwrap();
        let after = b.find(r#"write_raw("after")"#).unwrap();
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
}
