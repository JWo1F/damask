//! Introspect the `.rs` file paired with a `.dmk` template to learn a
//! component's fields and methods, for in-tag completion and hover.
//!
//! This is a lightweight `syn` parse — names and rendered types only, no type
//! resolution. It agrees with the macro on pairing by reusing
//! [`damask_template::to_snake_case`].

use damask_template::{ElementKind, Node, parse, to_snake_case};
use std::path::{Path, PathBuf};
use syn::{Attribute, Expr, ImplItem, Item, Lit, Meta, Type};

/// A field or method a template can reference through `self`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub name: String,
    /// Rendered type (fields) or signature (methods), for completion detail.
    pub detail: String,
    pub is_method: bool,
    /// The item's doc comment, if any — shown in hover and completion.
    pub docs: Option<String>,
    /// A field a caller may leave out: an `Option<_>`, or any field of a
    /// `#[component(default)]` struct. Meaningless for methods (always false).
    pub optional: bool,
}

/// One `<slot>` a component declares in its template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotDef {
    /// The slot's name; `None` is the default (unnamed) slot.
    pub name: Option<String>,
}

/// A `#[derive(Component)]` struct found in the crate, for element/attribute/use
/// completion.
#[derive(Debug, Clone)]
pub struct ComponentDef {
    pub name: String,
    pub fields: Vec<Member>,
    /// Best-effort `crate::…::Name` path, from the file location.
    pub module_path: String,
    /// The struct's own doc comment, if any.
    pub docs: Option<String>,
    /// The paired `.dmk`, if one sits beside the `.rs`. Kept as a path rather
    /// than parsed eagerly: only slot hover and slot completion need the
    /// declarations, so [`slots`](Self::slots) reads it on demand rather than on
    /// every completion keystroke that lists components.
    pub template_path: Option<PathBuf>,
}

impl ComponentDef {
    /// The slots this component's template declares (`<slot>` / `<slot name>`),
    /// read from [`template_path`](Self::template_path). Empty when there is no
    /// paired template or it does not parse.
    pub fn slots(&self) -> Vec<SlotDef> {
        let Some(path) = &self.template_path else {
            return Vec::new();
        };
        let Ok(src) = std::fs::read_to_string(path) else {
            return Vec::new();
        };
        let Ok(template) = parse(&src) else {
            return Vec::new();
        };
        let mut slots = Vec::new();
        collect_slots(&template.nodes, &mut slots);
        slots
    }
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
            collect_components_in_source(&text, &module, &path, out);
        }
    }
}

fn collect_components_in_source(
    text: &str,
    module: &str,
    rs_path: &Path,
    out: &mut Vec<ComponentDef>,
) {
    let Ok(file) = syn::parse_file(text) else {
        return;
    };
    for item in &file.items {
        if let Item::Struct(s) = item
            && has_component_derive(&s.attrs)
        {
            // `#[component(default)]` makes every prop skippable — the ones a
            // call site omits come from the struct's own `Default`.
            let all_default = has_component_default(&s.attrs);
            let fields = s
                .fields
                .iter()
                .filter_map(|f| {
                    f.ident.as_ref().map(|id| Member {
                        name: id.to_string(),
                        detail: normalize_ws(&render(&f.ty)),
                        is_method: false,
                        docs: doc_string(&f.attrs),
                        optional: all_default || is_option(&f.ty),
                    })
                })
                .collect();
            let name = s.ident.to_string();
            let template_path = sibling_template(rs_path, &name);
            out.push(ComponentDef {
                module_path: format!("{module}::{name}"),
                name,
                fields,
                docs: doc_string(&s.attrs),
                template_path,
            });
        }
    }
}

/// The paired `.dmk` beside `rs_path` for a component — the sibling
/// `<snake_case>.dmk` — when it exists. A path join and an `exists` check only;
/// the file is parsed later, and only if its slots are needed.
fn sibling_template(rs_path: &Path, struct_name: &str) -> Option<PathBuf> {
    let dmk = rs_path
        .parent()?
        .join(format!("{}.dmk", to_snake_case(struct_name)));
    dmk.is_file().then_some(dmk)
}

/// Walk a node tree collecting every `<slot>` declaration, in source order,
/// dropping duplicates by name so a slot filled and forwarded is listed once.
fn collect_slots(nodes: &[Node], out: &mut Vec<SlotDef>) {
    for node in nodes {
        match node {
            Node::Element(el) => {
                if el.kind == ElementKind::Slot {
                    let name = el
                        .attrs
                        .iter()
                        .find(|a| a.name.as_str() == "name")
                        .and_then(|a| a.value.as_static_text())
                        .map(str::to_string);
                    let def = SlotDef { name };
                    if !out.contains(&def) {
                        out.push(def);
                    }
                }
                collect_slots(&el.children, out);
            }
            Node::If(if_node) => {
                for (_, body) in &if_node.branches {
                    collect_slots(body, out);
                }
                if let Some(body) = &if_node.otherwise {
                    collect_slots(body, out);
                }
            }
            Node::For(f) => collect_slots(&f.body, out),
            Node::Snippet(s) => collect_slots(&s.body, out),
            _ => {}
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

/// Whether the struct carries `#[component(default)]`, which makes every prop
/// skippable.
fn has_component_default(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("component") {
            return false;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                found = true;
            }
            Ok(())
        });
        found
    })
}

/// Whether a type is written as an `Option<…>` — a prop a caller may omit,
/// arriving as `None`. A textual check on the outermost segment, which is all a
/// `syn` type without resolution can offer (`std::option::Option` included).
fn is_option(ty: &Type) -> bool {
    matches!(ty, Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "Option"))
}

/// The doc comment of an item, gathered from its `#[doc = "…"]` attributes (what
/// `///` desugars to), joined with newlines and trimmed. `None` when undocumented.
fn doc_string(attrs: &[Attribute]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let Meta::NameValue(nv) = &attr.meta
            && let Expr::Lit(expr) = &nv.value
            && let Lit::Str(s) = &expr.lit
        {
            // `///` keeps a leading space (`/// text` → `" text"`); drop just
            // that one so the rendered markdown is not uniformly indented.
            let line = s.value();
            lines.push(line.strip_prefix(' ').unwrap_or(&line).to_string());
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(lines.join("\n").trim().to_string())
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

/// The workspace root to launch rust-analyzer at: the *outermost* ancestor whose
/// `Cargo.toml` declares a `[workspace]`. One server rooted there analyses every
/// member crate, so a multi-crate workspace runs a single rust-analyzer rather
/// than one per member — the difference between ~200 MB and several times that.
/// Falls back to the nearest `Cargo.toml` for a standalone crate.
pub fn project_root(path: &Path) -> Option<PathBuf> {
    let mut dir = path.parent()?;
    let mut nearest: Option<PathBuf> = None;
    let mut workspace: Option<PathBuf> = None;
    loop {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() {
            if nearest.is_none() {
                nearest = Some(dir.to_path_buf());
            }
            // Keep climbing past a match so a nested workspace yields the
            // outermost root, which is the one that owns all the members.
            if std::fs::read_to_string(&manifest).is_ok_and(|text| declares_workspace(&text)) {
                workspace = Some(dir.to_path_buf());
            }
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }
    workspace.or(nearest)
}

/// Whether a `Cargo.toml`'s text declares a `[workspace]` (or `[workspace.*]`)
/// table — the marker of a workspace root, as opposed to a member crate.
fn declares_workspace(cargo_toml: &str) -> bool {
    cargo_toml.lines().any(|line| {
        let t = line.trim_start();
        t == "[workspace]" || t.starts_with("[workspace]") || t.starts_with("[workspace.")
    })
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
            let all_default = has_component_default(&s.attrs);
            for field in &s.fields {
                if let Some(id) = &field.ident {
                    members.push(Member {
                        name: id.to_string(),
                        detail: normalize_ws(&render(&field.ty)),
                        is_method: false,
                        docs: doc_string(&field.attrs),
                        optional: all_default || is_option(&field.ty),
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
                        docs: doc_string(&m.attrs),
                        optional: false,
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

    const DOCUMENTED: &str = r#"
        use damask::Component;

        /// A dismissible notice.
        #[derive(Component)]
        pub struct Notice {
            /// The headline.
            pub title: String,
            pub detail: Option<String>,
        }
    "#;

    #[test]
    fn captures_docs_and_optionality() {
        let info = introspect_source(DOCUMENTED, "notice").expect("found component");
        let title = info.members.iter().find(|m| m.name == "title").unwrap();
        assert_eq!(title.docs.as_deref(), Some("The headline."));
        assert!(!title.optional, "a plain String field is required");

        let detail = info.members.iter().find(|m| m.name == "detail").unwrap();
        assert!(detail.optional, "an Option field is skippable");
    }

    #[test]
    fn component_default_makes_every_field_optional() {
        let src = r#"
            #[derive(Component)]
            #[component(default)]
            pub struct Theme { pub accent: String, pub dense: bool }
        "#;
        let info = introspect_source(src, "theme").expect("found component");
        assert!(info.members.iter().all(|m| m.optional));
    }

    #[test]
    fn collects_slots_named_and_default() {
        // The showcase `Frame` shape: a default slot and a named one.
        let template = parse(
            r#"<section><slot/><footer><slot name="footer">© anon</slot></footer></section>"#,
        )
        .unwrap();
        let mut slots = Vec::new();
        collect_slots(&template.nodes, &mut slots);
        assert_eq!(
            slots,
            vec![
                SlotDef { name: None },
                SlotDef {
                    name: Some("footer".into())
                }
            ]
        );
    }

    #[test]
    fn collects_slots_through_control_flow_and_dedupes() {
        let template = parse(
            r#"{#if self.ok}<slot name="a"/>{:else}<slot name="a"/>{/if}{#for x in xs}<slot name="b"/>{/for}"#,
        )
        .unwrap();
        let mut slots = Vec::new();
        collect_slots(&template.nodes, &mut slots);
        // `a` appears twice in the source but is one slot; `b` is inside a loop.
        assert_eq!(
            slots,
            vec![
                SlotDef {
                    name: Some("a".into())
                },
                SlotDef {
                    name: Some("b".into())
                }
            ]
        );
    }

    /// End-to-end over the real showcase `Frame`: fields, and slots read from
    /// its paired `.dmk`. Ignored by default — it depends on the workspace
    /// layout, which is not present in every build environment.
    #[test]
    #[ignore = "reads the showcase example from the workspace tree"]
    fn showcase_frame_fields_and_slots() {
        let frame =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/showcase/src/frame.rs");
        let components = crate_components(&frame);
        let frame_def = components
            .iter()
            .find(|c| c.name == "Frame")
            .expect("Frame component");
        assert!(frame_def.fields.iter().any(|f| f.name == "title"));
        let slots = frame_def.slots();
        assert!(slots.contains(&SlotDef { name: None }), "default slot");
        assert!(
            slots.contains(&SlotDef {
                name: Some("footer".into())
            }),
            "footer slot from frame.dmk"
        );
    }

    #[test]
    fn workspace_manifest_is_recognized() {
        assert!(declares_workspace(
            "[workspace]\nmembers = [\"a\", \"b\"]\n[workspace.package]\nversion = \"1\"\n"
        ));
        assert!(declares_workspace("  [workspace.dependencies]\n"));
        // A member crate that only *inherits* from the workspace is not a root.
        assert!(!declares_workspace(
            "[package]\nname = \"member\"\nversion.workspace = true\n"
        ));
        // A commented-out header does not count.
        assert!(!declares_workspace("# [workspace]\n"));
    }

    #[test]
    fn doc_string_joins_multiline() {
        let attrs: syn::ItemStruct = syn::parse_quote! {
            /// First line.
            /// Second line.
            struct X;
        };
        assert_eq!(
            doc_string(&attrs.attrs).as_deref(),
            Some("First line.\nSecond line.")
        );
    }
}
