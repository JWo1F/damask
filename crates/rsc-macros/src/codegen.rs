use crate::resolve::resolve;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use rsc_template::{Node, TagKind, Template, to_snake_case};
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
    // multiple `<% %>` tags balance as a single Rust block.
    let body: TokenStream = match body_src.parse() {
        Ok(ts) => ts,
        Err(e) => {
            return compile_error(
                name.span(),
                &format!(
                    "in template `{}`: generated Rust did not parse ({e}). \
                     Check the Rust inside the `<% … %>` tags.",
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

/// Assemble the body of `render_into` as a string of Rust source.
///
/// Every tag becomes Rust that writes into `__rsc`; blocks (`{#if}`, `{#each}`,
/// `{#snippet}`) open/close ordinary Rust braces, so the whole thing parses as
/// one balanced block.
fn build_render_body(template: &Template) -> Result<String, String> {
    // Bring `Component`/`Render` into scope (unnamed) so `child.render()` and
    // other trait-method calls in a template resolve without the author having
    // to import the traits themselves.
    let mut body =
        String::from("{\n#[allow(unused_imports)] use ::rsc::{Component as _, Render as _};\n");
    for node in &template.nodes {
        match node {
            Node::Text { text, .. } => {
                // `{:?}` renders a valid, fully escaped Rust string literal.
                body.push_str(&format!("__rsc.write_raw({text:?});\n"));
            }
            Node::Tag { kind, code, .. } => match kind {
                TagKind::Expr => {
                    require_expr(code, "{ … }")?;
                    body.push_str(&format!("__rsc.write_escaped(&({code}));\n"));
                }
                TagKind::Html => {
                    require_expr(code, "{@html … }")?;
                    body.push_str(&format!("__rsc.write_display_raw(&({code}));\n"));
                }
                TagKind::Const => {
                    require_expr(code, "{@const … }")?;
                    body.push_str(&format!("let {code};\n"));
                }
                TagKind::Render => {
                    require_expr(code, "{@render … }")?;
                    body.push_str(&format!(
                        "::rsc::Render::render_into(&({code}), &mut *__rsc);\n"
                    ));
                }
                TagKind::If => {
                    require_expr(code, "{#if … }")?;
                    body.push_str(&format!("if {code} {{\n"));
                }
                TagKind::ElseIf => {
                    require_expr(code, "{:else if … }")?;
                    body.push_str(&format!("}} else if {code} {{\n"));
                }
                TagKind::Else => body.push_str("} else {\n"),
                TagKind::Each => body.push_str(&translate_each(code)?),
                TagKind::Snippet => body.push_str(&translate_snippet(code)?),
                TagKind::Close => {
                    // A snippet is a closure passed to `fragment(…)`, so its
                    // close must also end the call and the `let`.
                    if code == "snippet" {
                        body.push_str("});\n");
                    } else {
                        body.push_str("}\n");
                    }
                }
            },
        }
    }
    body.push_str("}\n");
    Ok(body)
}

/// `{#each EXPR as PAT}` → `for PAT in EXPR {`, and
/// `{#each EXPR as PAT, IDX}` → `for (IDX, PAT) in (EXPR).into_iter().enumerate() {`.
fn translate_each(code: &str) -> Result<String, String> {
    let (expr, binding) = code.split_once(" as ").ok_or_else(|| {
        format!("`{{#each {code}}}` needs `as`: `{{#each EXPR as pattern}}`")
    })?;
    let expr = expr.trim();
    let binding = binding.trim();
    if expr.is_empty() || binding.is_empty() {
        return Err(format!("malformed `{{#each {code}}}`"));
    }

    // A trailing `, ident` is the index form; anything else (e.g. a tuple
    // pattern `(a, b)`) is treated as the whole pattern.
    if let Some((pat, idx)) = binding.rsplit_once(',') {
        let (pat, idx) = (pat.trim(), idx.trim());
        let is_ident = !idx.is_empty()
            && idx.chars().all(|c| c.is_alphanumeric() || c == '_')
            && idx.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_');
        if is_ident && !pat.is_empty() {
            return Ok(format!(
                "for ({idx}, {pat}) in ({expr}).into_iter().enumerate() {{\n"
            ));
        }
    }
    Ok(format!("for {binding} in {expr} {{\n"))
}

/// `{#snippet name(params)}` → a `let` binding of a fragment closure. With
/// params it becomes a function returning a fragment (invoked as `name(args)`).
fn translate_snippet(code: &str) -> Result<String, String> {
    let open = code
        .find('(')
        .ok_or_else(|| format!("`{{#snippet {code}}}` needs `name(params)`"))?;
    let close = code
        .rfind(')')
        .ok_or_else(|| format!("`{{#snippet {code}}}` needs `name(params)`"))?;
    let name = code[..open].trim();
    let params = code[open + 1..close].trim();
    if name.is_empty() {
        return Err(format!("`{{#snippet {code}}}` needs a name"));
    }

    if params.is_empty() {
        Ok(format!(
            "let {name} = ::rsc::fragment(|__rsc: &mut dyn ::rsc::Renderer| {{\n"
        ))
    } else {
        Ok(format!(
            "let {name} = |{params}| ::rsc::fragment(move |__rsc: &mut dyn ::rsc::Renderer| {{\n"
        ))
    }
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
    use super::{build_render_body, translate_each, translate_snippet};

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
    fn directives_and_if() {
        assert!(body("{@html self.body}").contains("write_display_raw(&(self.body))"));
        assert!(body("{@const x = 1}").contains("let x = 1;"));
        assert!(body("{@render self.footer}").contains("::rsc::Render::render_into(&(self.footer)"));
        let b = body("{#if self.a}x{:else}y{/if}");
        assert!(b.contains("if self.a {"));
        assert!(b.contains("} else {"));
    }

    #[test]
    fn each_translation() {
        assert_eq!(
            translate_each("&self.items as item").unwrap(),
            "for item in &self.items {\n"
        );
        assert_eq!(
            translate_each("&self.items as item, i").unwrap(),
            "for (i, item) in (&self.items).into_iter().enumerate() {\n"
        );
        // A tuple pattern is not mistaken for the index form.
        assert_eq!(
            translate_each("self.pairs.iter() as (a, b)").unwrap(),
            "for (a, b) in self.pairs.iter() {\n"
        );
    }

    #[test]
    fn snippet_translation() {
        assert_eq!(
            translate_snippet("hero()").unwrap(),
            "let hero = ::rsc::fragment(|__rsc: &mut dyn ::rsc::Renderer| {\n"
        );
        assert!(
            translate_snippet("row(item: &Product)")
                .unwrap()
                .starts_with("let row = |item: &Product| ::rsc::fragment(move")
        );
    }

    #[test]
    fn empty_expression_is_an_error() {
        let err = build_render_body(&rsc_template::parse("{ }").unwrap()).unwrap_err();
        assert!(err.contains("empty expression"), "unexpected: {err}");
    }
}
