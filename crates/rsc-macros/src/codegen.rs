use crate::resolve::resolve;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use rsc_template::{Node, ParseOptions, TagKind, Template};
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

    let template = match rsc_template::parse(&resolved.source, &ParseOptions::default()) {
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
    let renderer = syn::Ident::new(resolved.host.renderer_type(), Span::call_site());
    let path_lit = LitStr::new(&resolved.path.to_string_lossy(), Span::call_site());

    quote! {
        impl #impl_generics ::rsc::Render for #name #ty_generics #where_clause {
            fn render_into(&self, __rsc: &mut dyn ::rsc::Renderer) #body
        }

        impl #impl_generics ::rsc::Component for #name #ty_generics #where_clause {
            fn default_renderer(&self) -> ::std::boxed::Box<dyn ::rsc::Renderer> {
                ::std::boxed::Box::new(::rsc::renderers::#renderer::new())
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
fn build_render_body(template: &Template) -> Result<String, String> {
    // Bring `Component`/`Render` into scope (unnamed) so `<%- child.render() %>`
    // and other trait-method calls in a template resolve without the author
    // having to import the traits themselves.
    let mut body = String::from(
        "{\n#[allow(unused_imports)] use ::rsc::{Component as _, Render as _};\n",
    );
    for node in &template.nodes {
        match node {
            Node::Text { text, .. } => {
                // `{:?}` renders a valid, fully escaped Rust string literal.
                body.push_str(&format!("__rsc.write_raw({text:?});\n"));
            }
            Node::Tag { kind, code, .. } => match kind {
                TagKind::Comment => {}
                TagKind::Escaped => {
                    require_expr(code, "<%= … %>")?;
                    body.push_str(&format!("__rsc.write_escaped(&({code}));\n"));
                }
                TagKind::Raw => {
                    require_expr(code, "<%- … %>")?;
                    body.push_str(&format!("__rsc.write_display_raw(&({code}));\n"));
                }
                TagKind::Render => {
                    require_expr(code, "<%+ … %>")?;
                    body.push_str(&format!(
                        "::rsc::Render::render_into(&({code}), &mut *__rsc);\n"
                    ));
                }
                TagKind::Statement => {
                    body.push_str(code);
                    body.push('\n');
                }
            },
        }
    }
    body.push_str("}\n");
    Ok(body)
}

fn require_expr(code: &str, tag: &str) -> Result<(), String> {
    if code.trim().is_empty() {
        Err(format!("empty expression in `{tag}` tag"))
    } else {
        Ok(())
    }
}

fn compile_error(span: Span, msg: &str) -> TokenStream {
    syn::Error::new(span, msg).to_compile_error()
}

/// Convert a `PascalCase` component name to `snake_case` for the file-name
/// convention. Handles acronym boundaries (`HTMLPage` → `html_page`).
fn to_snake_case(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                let next_is_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
                if prev.is_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_uppercase() && next_is_lower)
                {
                    out.push('_');
                }
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::to_snake_case;

    #[test]
    fn snake_case_handles_common_shapes() {
        assert_eq!(to_snake_case("Greeting"), "greeting");
        assert_eq!(to_snake_case("MyButton"), "my_button");
        assert_eq!(to_snake_case("HTMLPage"), "html_page");
        assert_eq!(to_snake_case("Card2Col"), "card2_col");
        assert_eq!(to_snake_case("already_snake"), "already_snake");
    }
}
