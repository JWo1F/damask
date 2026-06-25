use rsc_template::HostLang;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// A located template: its absolute path, contents, and host language.
pub struct Resolved {
    pub path: PathBuf,
    pub source: String,
    pub host: HostLang,
}

/// Resolve the template for a component.
///
/// With `explicit`, the path is resolved against `CARGO_MANIFEST_DIR` (then its
/// `src/` subdirectory as a fallback). Otherwise the crate is scanned for a file
/// whose component-basename equals `name_snake` (see [`component_basename`]).
pub fn resolve(name_snake: &str, explicit: Option<&str>) -> Result<Resolved, String> {
    let manifest = manifest_dir()?;

    if let Some(rel) = explicit {
        let direct = manifest.join(rel);
        let path = if direct.is_file() {
            direct
        } else {
            manifest.join("src").join(rel)
        };
        if !path.is_file() {
            return Err(format!(
                "template `{rel}` not found (looked in `{}` and its `src/` subdirectory)",
                manifest.display()
            ));
        }
        return read_resolved(path);
    }

    let candidates = scan(&manifest);
    let matches: Vec<&PathBuf> = candidates
        .iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(component_basename)
                .map(|base| base == name_snake)
                .unwrap_or(false)
        })
        .collect();

    match matches.as_slice() {
        [] => Err(format!(
            "no template found for component `{name_snake}`: expected a file named \
             `{name_snake}.<lang>.rsc` (e.g. `{name_snake}.html.rsc`) somewhere under `{}`. \
             Create one, or set `template = \"…\"` in the component.",
            manifest.display()
        )),
        [one] => read_resolved((*one).clone()),
        many => {
            let list = many
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            Err(format!(
                "multiple templates match component `{name_snake}`: {list}. \
                 Disambiguate with `template = \"…\"`."
            ))
        }
    }
}

fn manifest_dir() -> Result<PathBuf, String> {
    std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "CARGO_MANIFEST_DIR is not set, so RSC cannot locate templates by convention. \
             Build with Cargo, or set `template = \"…\"` with a path relative to the crate root."
                .to_string()
        })
}

fn read_resolved(path: PathBuf) -> Result<Resolved, String> {
    let source = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read template `{}`: {e}", path.display()))?;
    let host = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(HostLang::from_filename)
        .unwrap_or(HostLang::Plain);
    Ok(Resolved { path, source, host })
}

/// The component-basename of a template file: its name with `.rsc` and the
/// middle language extension removed. `greeting.html.rsc` → `greeting`,
/// `notes.rsc` → `notes`. Returns `None` if the name is not a `.rsc` file.
pub fn component_basename(file: &str) -> Option<String> {
    let stem = file.strip_suffix(".rsc")?;
    let base = match stem.rsplit_once('.') {
        Some((base, _lang)) => base,
        None => stem,
    };
    Some(base.to_string())
}

/// Recursively list every `.rsc` file under `root`, memoized per crate compile.
///
/// The proc-macro server process is reused across invocations within a single
/// crate's compilation, so this walks the tree at most once per crate.
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
        // Skip build output and hidden directories (e.g. target/, .git/).
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
    use super::component_basename;

    #[test]
    fn basename_strips_language_and_rsc() {
        assert_eq!(component_basename("greeting.html.rsc").as_deref(), Some("greeting"));
        assert_eq!(component_basename("app.js.rsc").as_deref(), Some("app"));
        assert_eq!(component_basename("notes.rsc").as_deref(), Some("notes"));
        assert_eq!(component_basename("not-a-template.txt"), None);
    }
}
