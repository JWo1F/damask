use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// A located template: its absolute path and contents.
#[derive(Debug)]
pub struct Resolved {
    pub path: PathBuf,
    pub source: String,
}

/// Resolve the template for a component.
///
/// `source_file` is the path of the `.rs` file the derive was written in (from
/// `Span::local_file()`), relative to the crate root or absolute, or `None` when
/// the compiler could not map the span to a file.
///
/// Resolution order:
/// 1. `#[template(path)]` if given — relative to the source file's directory,
///    then the crate root, then `src/`.
/// 2. The sibling convention — `<name_snake>.*.rsc` in the source file's own
///    directory.
/// 3. Fallback — a crate-wide scan by basename (used when the source directory
///    is unknown or empty of matches).
pub fn resolve(
    source_file: Option<&Path>,
    name_snake: &str,
    explicit: Option<&str>,
) -> Result<Resolved, String> {
    let manifest = manifest_dir()?;
    let source_dir = source_dir(&manifest, source_file);

    if let Some(rel) = explicit {
        let mut tried = Vec::new();
        if let Some(dir) = &source_dir {
            tried.push(dir.join(rel));
        }
        tried.push(manifest.join(rel));
        tried.push(manifest.join("src").join(rel));
        for candidate in &tried {
            if candidate.is_file() {
                return read_resolved(candidate.clone());
            }
        }
        return Err(format!(
            "template `{rel}` not found; looked in {}",
            tried
                .iter()
                .map(|p| format!("`{}`", p.display()))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // Sibling convention: look next to the source file first.
    if let Some(dir) = &source_dir {
        let matches = dir_matches(dir, name_snake);
        match matches.len() {
            1 => return read_resolved(matches.into_iter().next().unwrap()),
            0 => {} // fall through to the crate-wide scan
            _ => {
                return Err(ambiguous_message(name_snake, &matches));
            }
        }
    }

    // Fallback: scan the whole crate by basename.
    let candidates = scan(&manifest);
    let matches: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|p| basename_matches(p, name_snake))
        .collect();

    match matches.len() {
        1 => read_resolved(matches.into_iter().next().unwrap()),
        0 => Err(format!(
            "no template found for component `{name_snake}`: expected a file named \
             `{name_snake}.rsc` next to the struct{}. \
             Create one, or set `#[template(path = \"…\")]`.",
            source_dir
                .as_ref()
                .map(|d| format!(" (in `{}`)", d.display()))
                .unwrap_or_default()
        )),
        _ => Err(ambiguous_message(name_snake, &matches)),
    }
}

fn ambiguous_message(name_snake: &str, matches: &[PathBuf]) -> String {
    let list = matches
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "multiple templates match component `{name_snake}`: {list}. \
         Disambiguate with `#[template(path = \"…\")]`."
    )
}

fn manifest_dir() -> Result<PathBuf, String> {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "CARGO_MANIFEST_DIR is not set, so RSC cannot locate templates. \
             Build with Cargo, or set `#[template(path = \"…\")]`."
                .to_string()
        })
}

/// The absolute directory containing the source `.rs` file, if known.
///
/// `Span::local_file()` yields a path relative to rustc's working directory —
/// the workspace root for a Cargo build, which is not necessarily the crate's
/// `CARGO_MANIFEST_DIR`. Try the working directory first, then the manifest
/// dir, and only accept a base that actually locates the file, so a wrong guess
/// degrades to the crate-wide scan instead of a bogus directory.
fn source_dir(manifest: &Path, source_file: Option<&Path>) -> Option<PathBuf> {
    let file = source_file?;

    if file.is_absolute() {
        return file
            .is_file()
            .then(|| file.parent().map(Path::to_path_buf))?;
    }

    let mut bases = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd);
    }
    bases.push(manifest.to_path_buf());

    for base in bases {
        let abs = base.join(file);
        if abs.is_file() {
            return abs.parent().map(Path::to_path_buf);
        }
    }
    None
}

/// `.rsc` files directly in `dir` whose component-basename equals `name_snake`.
fn dir_matches(dir: &Path, name_snake: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && basename_matches(p, name_snake))
        .collect();
    out.sort();
    out
}

fn basename_matches(path: &Path, name_snake: &str) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(component_basename)
        .map(|base| base == name_snake)
        .unwrap_or(false)
}

fn read_resolved(path: PathBuf) -> Result<Resolved, String> {
    let source = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read template `{}`: {e}", path.display()))?;
    Ok(Resolved { path, source })
}

/// The component-basename of a template file: its name without the `.rsc`
/// extension (`greeting.rsc` → `greeting`). Returns `None` for non-`.rsc` files.
pub fn component_basename(file: &str) -> Option<String> {
    file.strip_suffix(".rsc").map(str::to_string)
}

/// Recursively list every `.rsc` file under `root`, memoized per crate compile.
fn scan(root: &Path) -> Vec<PathBuf> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Vec<PathBuf>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(hit) = cache.lock().unwrap().get(root) {
        return hit.clone();
    }

    let mut found = Vec::new();
    walk(root, &mut found);
    found.sort();

    cache
        .lock()
        .unwrap()
        .insert(root.to_path_buf(), found.clone());
    found
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name == "target" || name.starts_with('.') {
                continue;
            }
            walk(&path, out);
        } else if name.ends_with(".rsc") {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{component_basename, resolve};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn basename_strips_rsc() {
        assert_eq!(component_basename("greeting.rsc").as_deref(), Some("greeting"));
        assert_eq!(component_basename("my_button.rsc").as_deref(), Some("my_button"));
        assert_eq!(component_basename("not-a-template.txt"), None);
    }

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A fresh empty temp directory (no external crates; unique via pid+counter).
    fn unique_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("rsc_resolve_{}_{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_sibling_template_by_name() {
        let dir = unique_dir();
        let rs = dir.join("widget.rs");
        std::fs::write(&rs, "// component").unwrap();
        std::fs::write(dir.join("widget.rsc"), "hi {self.x}").unwrap();

        let resolved = resolve(Some(&rs), "widget", None).expect("should resolve");
        assert_eq!(resolved.source, "hi {self.x}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn explicit_template_path_wins() {
        let dir = unique_dir();
        let rs = dir.join("widget.rs");
        std::fs::write(&rs, "// component").unwrap();
        std::fs::write(dir.join("custom.rsc"), "<b>{self.x}</b>").unwrap();

        let resolved = resolve(Some(&rs), "widget", Some("custom.rsc")).expect("resolves");
        assert_eq!(resolved.source, "<b>{self.x}</b>");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_template_is_an_error() {
        let dir = unique_dir();
        let rs = dir.join("nope.rs");
        std::fs::write(&rs, "// component").unwrap();

        let err = resolve(Some(&rs), "nope", None).unwrap_err();
        assert!(err.contains("no template found"), "unexpected: {err}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
