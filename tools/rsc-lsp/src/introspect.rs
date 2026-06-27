//! Introspect the `.rs` file paired with a `.rsc` template to learn a
//! component's fields and methods, for in-tag completion and hover.
//!
//! This is a lightweight `syn` parse — names and rendered types only, no type
//! resolution. It agrees with the macro on pairing by reusing
//! [`rsc_template::to_snake_case`].

use rsc_template::to_snake_case;
use std::path::{Path, PathBuf};
use syn::{ImplItem, Item, Type};

/// A field or method a template can reference through `self`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    /// Rendered type (fields) or signature (methods), for completion detail.
    pub detail: String,
    pub is_method: bool,
}

/// The introspected component paired with a template.
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub struct_name: String,
    pub members: Vec<Member>,
}

/// Locate and introspect the component paired with the template at `rsc_path`.
pub fn for_template(rsc_path: &Path) -> Option<ComponentInfo> {
    let file_name = rsc_path.file_name()?.to_str()?;
    let basename = component_basename(file_name)?;
    let dir = rsc_path.parent()?;

    for rs in sibling_rs_files(dir, &basename) {
        if let Some(info) = introspect_file(&rs, &basename) {
            return Some(info);
        }
    }
    None
}

/// `.rs` files to try, `<basename>.rs` first, then the rest of the directory.
fn sibling_rs_files(dir: &Path, basename: &str) -> Vec<PathBuf> {
    let preferred = dir.join(format!("{basename}.rs"));
    let mut files = vec![preferred.clone()];
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path != preferred && path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
    files
}

fn component_basename(file: &str) -> Option<String> {
    let stem = file.strip_suffix(".rsc")?;
    Some(match stem.rsplit_once('.') {
        Some((base, _lang)) => base.to_string(),
        None => stem.to_string(),
    })
}

/// Parse `rs` and, if it defines a struct whose snake-cased name is `basename`,
/// collect that struct's fields and the methods of its inherent `impl`s.
pub fn introspect_file(rs: &Path, basename: &str) -> Option<ComponentInfo> {
    let text = std::fs::read_to_string(rs).ok()?;
    introspect_source(&text, basename)
}

/// Introspection over source text (unit-testable without touching the disk).
pub fn introspect_source(text: &str, basename: &str) -> Option<ComponentInfo> {
    let file = syn::parse_file(text).ok()?;

    let mut struct_ident = None;
    let mut members = Vec::new();

    for item in &file.items {
        if let Item::Struct(s) = item
            && to_snake_case(&s.ident.to_string()) == basename
        {
            struct_ident = Some(s.ident.clone());
            for field in &s.fields {
                if let Some(id) = &field.ident {
                    members.push(Member {
                        name: id.to_string(),
                        detail: render(&field.ty),
                        is_method: false,
                    });
                }
            }
            break;
        }
    }

    let ident = struct_ident?;

    for item in &file.items {
        if let Item::Impl(im) = item
            && im.trait_.is_none()
            && type_is_ident(&im.self_ty, &ident)
        {
            for impl_item in &im.items {
                if let ImplItem::Fn(m) = impl_item {
                    members.push(Member {
                        name: m.sig.ident.to_string(),
                        detail: normalize_ws(&render(&m.sig)),
                        is_method: true,
                    });
                }
            }
        }
    }

    Some(ComponentInfo {
        struct_name: ident.to_string(),
        members,
    })
}

fn type_is_ident(ty: &Type, ident: &syn::Ident) -> bool {
    matches!(ty, Type::Path(p) if p.path.segments.last().is_some_and(|s| &s.ident == ident))
}

fn render<T: quote::ToTokens>(t: &T) -> String {
    quote::quote!(#t).to_string()
}

/// `quote` inserts spaces around every token; collapse the worst of it so
/// completion detail reads like source.
fn normalize_ws(s: &str) -> String {
    s.replace(" :: ", "::")
        .replace(" < ", "<")
        .replace(" > ", ">")
        .replace(" ,", ",")
        .replace("& ", "&")
        .replace(" ( ", "(")
        .replace(" )", ")")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = r#"
        use rsc::Component;

        #[derive(Component)]
        pub struct Greeting {
            pub name: String,
            admin: bool,
        }

        impl Greeting {
            pub fn shout(&self) -> String { String::new() }
        }
    "#;

    #[test]
    fn collects_fields_and_methods() {
        let info = introspect_source(SRC, "greeting").expect("found component");
        assert_eq!(info.struct_name, "Greeting");
        let names: Vec<&str> = info.members.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"name"));
        assert!(names.contains(&"admin"));
        assert!(names.contains(&"shout"));

        let shout = info.members.iter().find(|m| m.name == "shout").unwrap();
        assert!(shout.is_method);
        assert!(shout.detail.contains("shout"));
    }

    #[test]
    fn returns_none_when_no_matching_struct() {
        assert!(introspect_source(SRC, "nonexistent").is_none());
    }
}
