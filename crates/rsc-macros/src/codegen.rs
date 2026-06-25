use crate::input::ComponentInput;
use crate::resolve::resolve;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use rsc_template::{Node, ParseOptions, TagKind, Template};
use syn::LitStr;

/// Expand a parsed `component!` into the struct, its inherent impl, and its
/// `Component` impl (whose `render_into` is generated from the template).
pub fn expand(input: ComponentInput) -> TokenStream {
    let name = input.name.clone();
    let name_snake = to_snake_case(&name.to_string());
    let explicit = input.template.as_ref().map(|l| l.value());

    let resolved = match resolve(&name_snake, explicit.as_deref()) {
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

    let vis = &input.vis;
    let fields = &input.fields;
    let impl_body = &input.impl_body;

    let renderer = syn::Ident::new(resolved.host.renderer_type(), Span::call_site());
    let path_lit = LitStr::new(&resolved.path.to_string_lossy(), Span::call_site());

    let inherent_impl = if impl_body.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #name {
                #impl_body
            }
        }
    };

    quote! {
        #vis struct #name {
            #(#fields),*
        }

        #inherent_impl

        impl ::rsc::Component for #name {
            fn render_into(&self, __rsc: &mut dyn ::rsc::Renderer) #body

            fn default_renderer(&self) -> ::std::boxed::Box<dyn ::rsc::Renderer> {
                ::std::boxed::Box::new(::rsc::renderers::#renderer::new())
            }
        }

        // Tie the template file into the crate's dependency graph so editing it
        // triggers a rebuild — no build script required.
        const _: &[::core::primitive::u8] = ::core::include_bytes!(#path_lit);
    }
}

/// Assemble the body of `render_into` as a string of Rust source.
///
/// Text and expression tags become `__rsc` calls; statement tags are spliced
/// verbatim; the whole thing is one balanced block parsed once by the caller.
fn build_render_body(template: &Template) -> Result<String, String> {
    let mut body = String::from("{\n");
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
                        "::rsc::Component::render_into(&({code}), &mut *__rsc);\n"
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
