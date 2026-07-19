use crate::resolve::resolve;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use damask_template::to_snake_case;
use std::path::PathBuf;
use syn::{Attribute, DeriveInput, LitStr};

/// Expand `#[derive(Component)]` into an `impl damask::Component` whose
/// `render_into` is generated from the struct's paired template, plus a private
/// `include_bytes!` binding so editing the template triggers a rebuild.
pub fn expand(input: DeriveInput, source_file: Option<PathBuf>) -> TokenStream {
    let name = input.ident.clone();

    let explicit = match extract_template_path(&input.attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error(),
    };

    let defaulted = match crate::props::extract_defaulted(&input.attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error(),
    };
    // Emitted whatever happens below: a template that fails to resolve or parse
    // is one error, and every call site reporting that the component cannot be
    // built would bury it.
    let builder = crate::props::expand(&input, defaulted);
    let failed = |span, msg: &str| {
        let error = compile_error(span, msg);
        quote! { #builder #error }
    };

    let name_snake = to_snake_case(&name.to_string());
    let resolved = match resolve(source_file.as_deref(), &name_snake, explicit.as_deref()) {
        Ok(r) => r,
        Err(msg) => return failed(name.span(), &msg),
    };

    let template = match damask_template::parse(&resolved.source) {
        Ok(t) => t,
        Err(e) => {
            return failed(
                name.span(),
                &format!("in template `{}`: {e}", resolved.path.display()),
            );
        }
    };

    // The template → Rust lowering lives in `damask-template` so the language
    // server generates byte-identical code for its virtual files.
    let body_src = match damask_template::lower(&template) {
        Ok(src) => src,
        Err(msg) => {
            return failed(
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
            return failed(
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
        impl #impl_generics ::damask::Render for #name #ty_generics #where_clause {
            fn render_into(&self, __damask: &mut dyn ::damask::Renderer) {
                ::damask::Render::render_slots(self, __damask, ::damask::Slots::EMPTY)
            }

            fn render_slots(
                &self,
                __damask: &mut dyn ::damask::Renderer,
                __damask_slots: ::damask::Slots<'_>,
            ) #body
        }

        impl #impl_generics ::damask::Component for #name #ty_generics #where_clause {
            fn default_renderer(&self) -> ::std::boxed::Box<dyn ::damask::Renderer> {
                ::std::boxed::Box::new(::damask::renderers::HtmlRenderer::new())
            }
        }

        // The builder a template's `<Name …/>` goes through, since a call site
        // cannot see which props it left out.
        #builder

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

fn compile_error(span: Span, msg: &str) -> TokenStream {
    syn::Error::new(span, msg).to_compile_error()
}
