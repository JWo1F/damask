//! Introspect the `.rs` file paired with a `.dmk` template to learn a
//! component's fields and methods, for in-tag completion and hover.
//!
//! This is a lightweight `syn` parse — names and rendered types only, no type
//! resolution. It agrees with the macro on pairing by reusing
//! [`damask_template::to_snake_case`].

use damask_template::to_snake_case;
use std::path::{Path, PathBuf};
use syn::{Attribute, ImplItem, Item, Type};

/// A field or method a template can reference through `self`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    /// Rendered type (fields) or signature (methods), for completion detail.
    pub detail: String,
    pub is_method: bool,
}

/// A `#[derive(Component)]` struct found in the crate, for element/attribute/use
/// completion.
#[derive(Debug, Clone)]
pub struct ComponentDef {
    pub name: String,
    pub fields: Vec<Member>,
    /// Best-effort `crate::…::Name` path, from the file location.
    pub module_path: String,
}

/// All components defined in the crate containing `from_file` (best-effort scan
/// of the crate's `src/`).
pub fn crate_components(from_file: &Path) -> Vec<ComponentDef> {
    let Some(src_root) = crate_src_dir(from_file) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_rs(&src_root, &src_root, &mut out);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Walk up from `from` to the crate root (first `Cargo.toml`) and return its
/// `src/` directory.
fn crate_src_dir(from: &Path) -> Option<PathBuf> {
    let mut dir = from.parent()?;
    loop {
        if dir.join("Cargo.toml").is_file() {
            let src = dir.join("src");
            return Some(if src.is_dir() { src } else { dir.to_path_buf() });
        }
        dir = dir.parent()?;
    }
}

fn collect_rs(src_root: &Path, dir: &Path, out: &mut Vec<ComponentDef>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name != "target" && !name.starts_with('.') {
                collect_rs(src_root, &path, out);
            }
        } else if name.ends_with(".rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            let module = module_path_for(src_root, &path);
            collect_components_in_source(&text, &module, out);
        }
    }
}

fn collect_components_in_source(text: &str, module: &str, out: &mut Vec<ComponentDef>) {
    let Ok(file) = syn::parse_file(text) else {
        return;
    };
    for item in &file.items {
        if let Item::Struct(s) = item
            && has_component_derive(&s.attrs)
        {
            let fields = s
                .fields
                .iter()
                .filter_map(|f| {
                    f.ident.as_ref().map(|id| Member {
                        name: id.to_string(),
                        detail: render(&f.ty),
                        is_method: false,
                    })
                })
                .collect();
            let name = s.ident.to_string();
            out.push(ComponentDef {
                module_path: format!("{module}::{name}"),
                name,
                fields,
            });
        }
    }
}

fn has_component_derive(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("Component") {
                found = true;
            }
            Ok(())
        });
        found
    })
}

/// Best-effort `crate::…` module path for a file under `src_root`.
fn module_path_for(src_root: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(src_root).unwrap_or(file);
    let mut parts: Vec<String> = rel
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(String::from))
        .collect();
    if let Some(last) = parts.last_mut()
        && let Some(stem) = last.strip_suffix(".rs")
    {
        if matches!(stem, "lib" | "main" | "mod") {
            parts.pop();
        } else {
            *last = stem.to_string();
        }
    }
    let mut path = String::from("crate");
    for p in parts {
        path.push_str("::");
        path.push_str(&p);
    }
    path
}

/// The introspected component paired with a template.
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub struct_name: String,
    pub members: Vec<Member>,
}

/// Locate and introspect the component paired with the template at `damask_path`.
pub fn for_template(damask_path: &Path) -> Option<ComponentInfo> {
    let file_name = damask_path.file_name()?.to_str()?;
    let basename = component_basename(file_name)?;
    let dir = damask_path.parent()?;

    for rs in sibling_rs_files(dir, &basename) {
        if let Some(info) = introspect_file(&rs, &basename) {
            return Some(info);
        }
    }
    None
}

/// Locate the `.rs` file paired with `damask_path` and the component struct it
/// defines, for building the rust-analyzer overlay. Like [`for_template`] but
/// returns the file path rather than the introspected members.
pub fn paired_rs(damask_path: &Path) -> Option<(PathBuf, String)> {
    let file_name = damask_path.file_name()?.to_str()?;
    let basename = component_basename(file_name)?;
    let dir = damask_path.parent()?;
    for rs in sibling_rs_files(dir, &basename) {
        if let Some(info) = introspect_file(&rs, &basename) {
            return Some((rs, info.struct_name));
        }
    }
    None
}

/// The nearest ancestor directory of `path` containing a `Cargo.toml` — the root
/// to launch rust-analyzer at.
pub fn project_root(path: &Path) -> Option<PathBuf> {
    let mut dir = path.parent()?;
    loop {
        if dir.join("Cargo.toml").is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
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
    file.strip_suffix(".dmk").map(str::to_string)
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
        use damask::Component;

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
