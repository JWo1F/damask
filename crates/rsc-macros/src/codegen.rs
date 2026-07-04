use crate::resolve::resolve;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use rsc_template::{
    AttrValue, EachNode, Element, ElementKind, IfNode, Node, SnippetNode, Template, is_void_element,
    to_snake_case,
};
use std::path::PathBuf;
use syn::{Attribute, DeriveInput, LitStr};

/// Expand `#[derive(Component)]` into an `impl rsc::Component` whose
/// `render_into` is generated from the struct's paired template, plus a private
/// `include_bytes!` binding so editing the template triggers a rebuild.
pub fn expand(input: DeriveInput, source_file: Option<PathBuf>) -> TokenStream {
    let name = input.ident.clone();

    let explicit = match extract_template_path(&input.attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error(),
    };

    let name_snake = to_snake_case(&name.to_string());
    let resolved = match resolve(source_file.as_deref(), &name_snake, explicit.as_deref()) {
        Ok(r) => r,
        Err(msg) => return compile_error(name.span(), &msg),
    };

    let template = match rsc_template::parse(&resolved.source) {
        Ok(t) => t,
        Err(e) => {
            return compile_error(
                name.span(),
                &format!("in template `{}`: {e}", resolved.path.display()),
            );
        }
    };

    let body_src = match build_render_body(&template) {
        Ok(src) => src,
        Err(msg) => {
            return compile_error(
                name.span(),
                &format!("in template `{}`: {msg}", resolved.path.display()),
            );
        }
    };

    // Parse the assembled body once, so control-flow tags whose braces span
    // multiple `{ }` block tags balance as a single Rust block.
    let body: TokenStream = match body_src.parse() {
        Ok(ts) => ts,
        Err(e) => {
            return compile_error(
                name.span(),
                &format!(
                    "in template `{}`: generated Rust did not parse ({e}). \
                     Check the Rust inside the `{{ … }}` tags.",
                    resolved.path.display()
                ),
            );
        }
    };

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let path_lit = LitStr::new(&resolved.path.to_string_lossy(), Span::call_site());

    quote! {
        impl #impl_generics ::rsc::Render for #name #ty_generics #where_clause {
            fn render_into(&self, __rsc: &mut dyn ::rsc::Renderer) #body
        }

        impl #impl_generics ::rsc::Component for #name #ty_generics #where_clause {
            fn default_renderer(&self) -> ::std::boxed::Box<dyn ::rsc::Renderer> {
                ::std::boxed::Box::new(::rsc::renderers::HtmlRenderer::new())
            }
        }

        // Tie the template file into the crate's dependency graph so editing it
        // triggers a rebuild — no build script required.
        const _: &[::core::primitive::u8] = ::core::include_bytes!(#path_lit);
    }
}

/// Read a `#[template(path = "…")]` helper attribute, if present.
fn extract_template_path(attrs: &[Attribute]) -> syn::Result<Option<String>> {
    let mut found = None;
    for attr in attrs {
        if !attr.path().is_ident("template") {
            continue;
        }
        let mut path: Option<String> = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("path") {
                let value: LitStr = meta.value()?.parse()?;
                path = Some(value.value());
                Ok(())
            } else {
                Err(meta.error("unknown `template` option; expected `path = \"…\"`"))
            }
        })?;
        match path {
            Some(p) => found = Some(p),
            None => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "`#[template]` requires `path = \"…\"`",
                ));
            }
        }
    }
    Ok(found)
}

/// Assemble the body of `render_into` as a string of Rust source, then the
/// caller parses it once so all the `{ }` blocks and HTML-element scope blocks
/// balance as a single Rust block.
fn build_render_body(template: &Template) -> Result<String, String> {
    // Bring `Component`/`Render` into scope (unnamed) so `child.render()` and
    // `{@render …}`-style calls resolve without the author importing the traits.
    let mut out =
        String::from("{\n#[allow(unused_imports)] use ::rsc::{Component as _, Render as _};\n");
    emit_nodes(&template.nodes, &mut out)?;
    out.push_str("}\n");
    Ok(out)
}

fn emit_nodes(nodes: &[Node], out: &mut String) -> Result<(), String> {
    for node in nodes {
        emit_node(node, out)?;
    }
    Ok(())
}

fn emit_node(node: &Node, out: &mut String) -> Result<(), String> {
    match node {
        // `{:?}` renders a valid, fully escaped Rust string literal.
        Node::Text(text) => out.push_str(&format!("__rsc.write_raw({text:?});\n")),
        Node::Expr(code) => emit_expr(code, out),
        Node::Html(code) => {
            require_expr(code, "{@html … }")?;
            out.push_str(&format!("__rsc.write_display_raw(&({code}));\n"));
        }
        Node::Render(code) => {
            require_expr(code, "{@render … }")?;
            out.push_str(&format!("::rsc::Render::render_into(&({code}), &mut *__rsc);\n"));
        }
        Node::Use(path) => {
            require_expr(path, "{#use … }")?;
            out.push_str(&format!("#[allow(unused_imports)] use {path};\n"));
        }
        Node::If(if_node) => emit_if(if_node, out)?,
        Node::Each(each) => emit_each(each, out)?,
        Node::Snippet(snippet) => emit_snippet(snippet, out)?,
        Node::Element(element) => emit_element(element, out)?,
    }
    Ok(())
}

/// A `{ … }` block: splice it as a statement (no output) if it's a binding or
/// ends in `;`, otherwise print its value (escaped).
fn emit_expr(code: &str, out: &mut String) {
    let trimmed = code.trim();
    if is_statement(trimmed) {
        out.push_str(trimmed);
        if !trimmed.ends_with(';') {
            out.push(';');
        }
        out.push('\n');
    } else if trimmed.contains(';') {
        // Multiple statements ending in an expression need a block; the block's
        // value is a temporary, so borrowing it is fine.
        out.push_str(&format!("__rsc.write_escaped(&({{ {trimmed} }}));\n"));
    } else {
        // A plain expression: borrow it directly (no block) so field access
        // like `self.name` borrows rather than moves out of `&self`.
        out.push_str(&format!("__rsc.write_escaped(&({trimmed}));\n"));
    }
}

/// Whether a `{ … }` block is a statement (yields no value to print).
fn is_statement(trimmed: &str) -> bool {
    trimmed.ends_with(';') || starts_with_kw(trimmed, "let") || starts_with_kw(trimmed, "const")
}

fn starts_with_kw(s: &str, kw: &str) -> bool {
    s.strip_prefix(kw)
        .and_then(|r| r.chars().next())
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
        && s.starts_with(kw)
}

fn emit_if(if_node: &IfNode, out: &mut String) -> Result<(), String> {
    for (i, (cond, body)) in if_node.branches.iter().enumerate() {
        require_expr(cond, "{#if … }")?;
        if i == 0 {
            out.push_str(&format!("if {cond} {{\n"));
        } else {
            out.push_str(&format!("}} else if {cond} {{\n"));
        }
        emit_nodes(body, out)?;
    }
    if let Some(otherwise) = &if_node.otherwise {
        out.push_str("} else {\n");
        emit_nodes(otherwise, out)?;
    }
    out.push_str("}\n");
    Ok(())
}

/// `{#each E as p}` → `for p in E {`, and `{#each E as p, i}` →
/// `for (i, p) in (E).into_iter().enumerate() {`.
fn emit_each(each: &EachNode, out: &mut String) -> Result<(), String> {
    let (expr, binding) = (each.expr.trim(), each.binding.trim());
    if expr.is_empty() || binding.is_empty() {
        return Err("malformed `{#each}`".into());
    }
    // A trailing `, ident` is the index form; anything else (e.g. a tuple
    // pattern `(a, b)`) is treated as the whole pattern.
    if let Some((pat, idx)) = binding.rsplit_once(',') {
        let (pat, idx) = (pat.trim(), idx.trim());
        let is_ident = !idx.is_empty()
            && idx.chars().all(|c| c.is_alphanumeric() || c == '_')
            && idx.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_');
        if is_ident && !pat.is_empty() {
            out.push_str(&format!("for ({idx}, {pat}) in ({expr}).into_iter().enumerate() {{\n"));
            emit_nodes(&each.body, out)?;
            out.push_str("}\n");
            return Ok(());
        }
    }
    out.push_str(&format!("for {binding} in {expr} {{\n"));
    emit_nodes(&each.body, out)?;
    out.push_str("}\n");
    Ok(())
}

fn emit_snippet(snippet: &SnippetNode, out: &mut String) -> Result<(), String> {
    if snippet.name.is_empty() {
        return Err("`{#snippet}` needs a name".into());
    }
    if snippet.params.is_empty() {
        out.push_str(&format!(
            "let {} = ::rsc::fragment(|__rsc: &mut dyn ::rsc::Renderer| {{\n",
            snippet.name
        ));
    } else {
        out.push_str(&format!(
            "let {} = |{}| ::rsc::fragment(move |__rsc: &mut dyn ::rsc::Renderer| {{\n",
            snippet.name, snippet.params
        ));
    }
    emit_nodes(&snippet.body, out)?;
    out.push_str("});\n");
    Ok(())
}

fn emit_element(el: &Element, out: &mut String) -> Result<(), String> {
    match el.kind {
        ElementKind::Html => emit_html_element(el, out),
        ElementKind::Component => emit_component_element(el, out),
        ElementKind::Slot => emit_slot_placeholder(el, out),
    }
}

/// Append `__rsc.write_raw("…")` for `raw`, if non-empty, and clear it.
fn flush_raw(raw: &mut String, out: &mut String) {
    if !raw.is_empty() {
        out.push_str(&format!("__rsc.write_raw({raw:?});\n"));
        raw.clear();
    }
}

fn emit_html_element(el: &Element, out: &mut String) -> Result<(), String> {
    let mut raw = String::new();
    raw.push('<');
    raw.push_str(&el.tag);

    for attr in &el.attrs {
        match &attr.value {
            AttrValue::Boolean => {
                raw.push(' ');
                raw.push_str(&attr.name);
            }
            AttrValue::Literal(v) => {
                raw.push(' ');
                raw.push_str(&attr.name);
                raw.push_str("=\"");
                raw.push_str(v);
                raw.push('"');
            }
            AttrValue::Expr(code) => {
                require_expr(code, "attribute value")?;
                raw.push(' ');
                raw.push_str(&attr.name);
                raw.push_str("=\"");
                flush_raw(&mut raw, out);
                out.push_str(&format!("__rsc.write_escaped(&({code}));\n"));
                raw.push('"');
            }
        }
    }

    if el.self_closing {
        if is_void_element(&el.tag) {
            raw.push('>');
        } else {
            raw.push_str(&format!("></{}>", el.tag));
        }
        flush_raw(&mut raw, out);
        return Ok(());
    }

    raw.push('>');
    flush_raw(&mut raw, out);

    // A scope block so `{#use}` (and bindings) are scoped to this element.
    out.push_str("{\n");
    emit_nodes(&el.children, out)?;
    out.push_str("}\n");

    out.push_str(&format!("__rsc.write_raw({:?});\n", format!("</{}>", el.tag)));
    Ok(())
}

/// `<slot/>` / `<slot name="x"/>` in a component template — render the passed slot.
fn emit_slot_placeholder(el: &Element, out: &mut String) -> Result<(), String> {
    let field = slot_field(el)?;
    out.push_str(&format!("::rsc::Render::render_into(&(self.{field}), &mut *__rsc);\n"));
    Ok(())
}

/// The component field a `<slot>` maps to: `children` for the default slot,
/// otherwise the `name="…"` attribute.
fn slot_field(el: &Element) -> Result<String, String> {
    match el.attrs.iter().find(|a| a.name == "name") {
        None => Ok("children".to_string()),
        Some(attr) => match &attr.value {
            AttrValue::Literal(name) => Ok(name.clone()),
            _ => Err("`<slot name>` must be a string literal".into()),
        },
    }
}

/// `<Comp attr={e}>…</Comp>` — build `Comp { attr: e, <slots> }` and render it.
fn emit_component_element(el: &Element, out: &mut String) -> Result<(), String> {
    // Partition children into named-slot fills and default-slot content.
    let mut default: Vec<&Node> = Vec::new();
    let mut named: Vec<(String, &[Node])> = Vec::new();
    for child in &el.children {
        match child {
            Node::Element(slot) if slot.kind == ElementKind::Slot => {
                let name = slot_field(slot)?;
                if name == "children" {
                    return Err("`<slot>` inside a component needs `name=\"…\"`".into());
                }
                named.push((name, &slot.children));
            }
            other => default.push(other),
        }
    }

    out.push_str("::rsc::Render::render_into(&(");
    out.push_str(&el.tag);
    out.push_str(" {\n");

    for attr in &el.attrs {
        match &attr.value {
            AttrValue::Expr(code) => {
                require_expr(code, "attribute value")?;
                out.push_str(&format!("{}: ({}),\n", attr.name, code));
            }
            AttrValue::Literal(v) => out.push_str(&format!("{}: {v:?}.into(),\n", attr.name)),
            AttrValue::Boolean => out.push_str(&format!("{}: true,\n", attr.name)),
        }
    }

    // Default slot: sent only when there is real (non-whitespace) content.
    let has_default = default
        .iter()
        .any(|n| !matches!(n, Node::Text(t) if t.trim().is_empty()));
    if has_default {
        out.push_str("children: ::rsc::fragment(|__rsc: &mut dyn ::rsc::Renderer| {\n");
        for n in &default {
            emit_node(n, out)?;
        }
        out.push_str("}),\n");
    }

    for (name, body) in &named {
        out.push_str(&format!(
            "{name}: ::rsc::fragment(|__rsc: &mut dyn ::rsc::Renderer| {{\n"
        ));
        emit_nodes(body, out)?;
        out.push_str("}),\n");
    }

    out.push_str("}), &mut *__rsc);\n");
    Ok(())
}

fn require_expr(code: &str, tag: &str) -> Result<(), String> {
    if code.trim().is_empty() {
        Err(format!("empty expression in `{tag}`"))
    } else {
        Ok(())
    }
}

fn compile_error(span: Span, msg: &str) -> TokenStream {
    syn::Error::new(span, msg).to_compile_error()
}

#[cfg(test)]
mod tests {
    use super::{build_render_body, is_statement};

    fn body(src: &str) -> String {
        build_render_body(&rsc_template::parse(src).unwrap()).unwrap()
    }

    #[test]
    fn text_and_expression() {
        let b = body("Hi {self.name}!");
        assert!(b.contains(r#"__rsc.write_raw("Hi ")"#));
        assert!(b.contains("__rsc.write_escaped(&(self.name))"));
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
        assert!(body("{2 + 3; 10}").contains("write_escaped(&({ 2 + 3; 10 }))"));
    }

    #[test]
    fn directives_and_use() {
        assert!(body("{@html self.body}").contains("write_display_raw(&(self.body))"));
        assert!(body("{@render self.footer}").contains("::rsc::Render::render_into(&(self.footer)"));
        assert!(body("{#use crate::Card}").contains("use crate::Card;"));
    }

    #[test]
    fn if_and_each() {
        let b = body("{#if self.a}x{:else}y{/if}");
        assert!(b.contains("if self.a {"));
        assert!(b.contains("} else {"));
        assert!(body("{#each &self.items as item}{item}{/each}").contains("for item in &self.items {"));
        assert!(
            body("{#each &self.items as item, i}{i}{/each}")
                .contains("for (i, item) in (&self.items).into_iter().enumerate() {")
        );
    }

    #[test]
    fn html_element_scopes_and_attrs() {
        let b = body(r#"<div id={self.id}>{#use crate::X}hi</div>"#);
        assert!(b.contains("__rsc.write_escaped(&(self.id))"));
        // element content is a scope block containing the use
        assert!(b.contains("use crate::X;"));
        assert!(b.contains(r#"write_raw("</div>")"#));
    }

    #[test]
    fn void_and_self_closing_elements() {
        assert!(body("<br>").contains(r#"write_raw("<br>")"#));
        assert!(body("<hr class=\"x\">").contains(r#"write_raw("<hr class=\"x\">")"#));
    }

    #[test]
    fn component_element_construction() {
        let b = body(r#"<Card title={2 + 8} tag="h1">body<slot name="foot">f</slot></Card>"#);
        assert!(b.contains("::rsc::Render::render_into(&(Card {"));
        assert!(b.contains("title: (2 + 8),"));
        assert!(b.contains(r#"tag: "h1".into(),"#));
        assert!(b.contains("children: ::rsc::fragment("));
        assert!(b.contains("foot: ::rsc::fragment("));
    }

    #[test]
    fn slot_placeholder_renders_field() {
        assert!(body("<slot/>").contains("::rsc::Render::render_into(&(self.children)"));
        assert!(body(r#"<slot name="foot"/>"#).contains("::rsc::Render::render_into(&(self.foot)"));
    }

    #[test]
    fn empty_tag_is_an_error() {
        // `{ }` is rejected at parse time.
        assert!(rsc_template::parse("{ }").is_err());
        // an empty `{@html}` is rejected at codegen time.
        let err = build_render_body(&rsc_template::parse("{@html }").unwrap()).unwrap_err();
        assert!(err.contains("empty expression"), "unexpected: {err}");
    }
}
