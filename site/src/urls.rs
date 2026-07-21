//! Where the site is mounted.
//!
//! A GitHub project page lives under `/<repo>/`, a user page and a custom domain
//! at the root. That is a deploy-time fact, so every link and asset reference is
//! built through here rather than written into a template — a template with `/`
//! in it is a template that only works on one of the two.

/// The path the site is served from, and the only thing that knows about it.
#[derive(Debug, Clone)]
pub struct Urls {
    /// Normalised to either `""` or `/segment` — no trailing slash, so joining
    /// is always `base + path` with `path` carrying the leading one.
    base: String,
}

impl Urls {
    pub fn new(base: &str) -> Self {
        let trimmed = base.trim().trim_matches('/');
        let base = if trimmed.is_empty() {
            String::new()
        } else {
            format!("/{trimmed}")
        };
        Self { base }
    }

    /// Rewrites a root-relative path for the deploy.
    ///
    /// `to("/")` is the one case worth naming: the root has to stay `/` when
    /// there is no base, and become `/damask/` — with the trailing slash — when
    /// there is, or the browser resolves relative assets one directory too high.
    pub fn to(&self, path: &str) -> String {
        debug_assert!(path.starts_with('/'), "paths passed to Urls are absolute");
        format!("{}{path}", self.base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_base_leaves_paths_alone() {
        let urls = Urls::new("");
        assert_eq!(urls.to("/"), "/");
        assert_eq!(urls.to("/book/intro/"), "/book/intro/");
    }

    #[test]
    fn a_base_is_normalised_however_it_is_written() {
        for written in ["damask", "/damask", "damask/", "/damask/"] {
            let urls = Urls::new(written);
            assert_eq!(urls.to("/"), "/damask/", "from {written:?}");
            assert_eq!(urls.to("/docs/"), "/damask/docs/", "from {written:?}");
        }
    }
}
