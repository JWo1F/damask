//! Internal link checking.
//!
//! Every in-site link is written root-relative in markdown or in a template, and
//! nothing else verifies that the other end exists — a slug comes from a
//! filename, a fragment comes from a heading, and neither is a symbol the
//! compiler can follow. A broken cross-reference is the failure mode a
//! documentation site actually has, so the build fails on one.
//!
//! The check runs over the *rendered* HTML rather than over the markdown source.
//! By then the base path has been applied and the heading anchors are the ones
//! Comrak really wrote, so what is verified is what a reader will click.

use std::collections::{HashMap, HashSet};

/// Everything the finished site offers as a destination.
#[derive(Default)]
pub struct Targets {
    /// Every href a page is served at, plus the assets copied alongside.
    paths: HashSet<String>,
    /// Path → the `id`s on that page, for checking fragments.
    anchors: HashMap<String, HashSet<String>>,
}

impl Targets {
    pub fn add_page(&mut self, href: &str, anchors: impl IntoIterator<Item = String>) {
        self.paths.insert(href.to_string());
        self.anchors
            .entry(href.to_string())
            .or_default()
            .extend(anchors);
    }

    pub fn add_asset(&mut self, href: &str) {
        self.paths.insert(href.to_string());
    }
}

/// Checks one page's links, returning a message per broken one.
pub fn check(source_href: &str, html: &str, targets: &Targets) -> Vec<String> {
    let mut broken = Vec::new();

    for link in hrefs(html) {
        // Only in-site links. An external URL is not this build's business, and
        // a bare fragment resolves against the page it is on.
        if !link.starts_with('/') {
            continue;
        }

        let (path, fragment) = match link.split_once('#') {
            Some((path, fragment)) => (path, Some(fragment)),
            None => (link.as_str(), None),
        };

        if !targets.paths.contains(path) {
            broken.push(format!("{source_href} → {link} (no such page)"));
            continue;
        }

        // An unknown fragment is only an error where the target is a page whose
        // anchors were recorded; an asset has none and never will.
        if let (Some(fragment), Some(anchors)) = (fragment, targets.anchors.get(path))
            && !anchors.contains(fragment)
        {
            broken.push(format!("{source_href} → {link} (no such heading)"));
        }
    }

    broken
}

/// Every `href="…"` in a document.
///
/// A regex-free scan, because the generator has no regex dependency and the
/// input is markup it produced itself — every href it emits is double-quoted, so
/// there is no attribute-quoting variation to accommodate.
fn hrefs(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;

    while let Some(at) = rest.find("href=\"") {
        rest = &rest[at + 6..];
        match rest.find('"') {
            Some(end) => {
                out.push(unescape(&rest[..end]));
                rest = &rest[end..];
            }
            None => break,
        }
    }

    out
}

/// Reverses the escaping the renderers apply to an attribute value.
///
/// Only the entities that can appear in a URL Damask or Comrak wrote. A link
/// with a query string is the common case — `&` reaches the markup as `&amp;`,
/// and comparing that against a target path would fail every time.
fn unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Targets {
        let mut targets = Targets::default();
        targets.add_page("/docs/slots/", ["filling-from-rust".to_string()]);
        targets.add_asset("/assets/site.css");
        targets
    }

    #[test]
    fn a_link_to_a_real_page_and_heading_passes() {
        let html = r#"<a href="/docs/slots/#filling-from-rust">x</a>"#;
        assert!(check("/book/why/", html, &targets()).is_empty());
    }

    #[test]
    fn a_missing_page_is_reported() {
        let html = r#"<a href="/docs/gone/">x</a>"#;
        let broken = check("/book/why/", html, &targets());
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(broken[0].contains("no such page"), "{broken:?}");
    }

    #[test]
    fn a_missing_heading_is_reported() {
        let html = r#"<a href="/docs/slots/#nope">x</a>"#;
        let broken = check("/book/why/", html, &targets());
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert!(broken[0].contains("no such heading"), "{broken:?}");
    }

    /// An asset has no anchors recorded, and a fragment on one is not an error
    /// this build can rule on.
    #[test]
    fn a_fragment_on_an_asset_is_not_checked() {
        let html = r#"<a href="/assets/site.css#anything">x</a>"#;
        assert!(check("/book/why/", html, &targets()).is_empty());
    }

    #[test]
    fn external_and_relative_links_are_left_alone() {
        // `r##` because the body contains `"#`, which would close an `r#` string.
        let html = r##"<a href="https://example.com/x">a</a><a href="#local">b</a>"##;
        assert!(check("/book/why/", html, &targets()).is_empty());
    }

    #[test]
    fn an_escaped_ampersand_is_compared_as_written() {
        assert_eq!(hrefs(r#"<a href="/x/?a=1&amp;b=2">"#), ["/x/?a=1&b=2"]);
    }
}
