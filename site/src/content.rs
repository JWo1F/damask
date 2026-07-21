//! The markdown database.
//!
//! Everything the site says lives in `content/` as markdown with TOML front
//! matter. This module turns that tree into the values the templates render, and
//! it is the only place that knows the tree's shape — a page's struct in
//! `view/pages/` names what a page *needs*, not where it came from.
//!
//! **Order comes from the filename, not the front matter.** A chapter is
//! `03-templates.md` and sorts by that prefix, which means the reading order is
//! visible in a directory listing and cannot disagree with itself. An `order`
//! field would be a second copy of the same fact, and the copies drift the first
//! time two chapters are swapped.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::highlight::Highlighter;
use crate::markdown::{self, Heading};
use crate::urls::Urls;

/// A content error, phrased for whoever is editing the markdown.
pub type Error = String;

/// Which of the two long-form surfaces a collection is.
///
/// They differ in how they are read, and that is what the templates key off:
/// a book is read front to back, so a chapter offers the next one; reference is
/// read by lookup, so a page offers its neighbours in a grouped sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Book,
    Docs,
}

impl Kind {
    pub fn dir(self) -> &'static str {
        match self {
            Kind::Book => "book",
            Kind::Docs => "docs",
        }
    }
}

/// One markdown document, rendered.
#[derive(Debug, Clone)]
pub struct Page {
    pub slug: String,
    pub href: String,
    pub title: String,
    /// The markdown source, for a `<meta>` description.
    pub summary: String,
    /// The same text rendered inline, for display.
    pub summary_html: String,
    /// The docs sidebar's grouping. `None` puts a page in the leading, unlabelled
    /// group — which is where an overview belongs, above the first heading.
    pub section: Option<String>,
    /// The rendered markdown body.
    pub body: String,
    pub headings: Vec<Heading>,
}

/// A run of pages read as one thing.
#[derive(Debug, Clone)]
pub struct Collection {
    pub kind: Kind,
    pub title: String,
    pub lede: String,
    pub lede_html: String,
    /// The landing page's prose, above the list of pages.
    pub intro: String,
    pub href: String,
    pub pages: Vec<Page>,
}

impl Collection {
    /// The pages grouped for the docs sidebar, in file order.
    ///
    /// Sections are ordered by where they first appear rather than by a number
    /// on each page: the file order already decides it, and a `section_order`
    /// field would let two pages in the same section disagree about where their
    /// section goes.
    pub fn sections(&self) -> Vec<Section> {
        let mut sections: Vec<Section> = Vec::new();
        for page in &self.pages {
            let label = page.section.clone().unwrap_or_default();
            match sections.iter_mut().find(|s| s.label == label) {
                Some(section) => section.pages.push(page.clone()),
                None => sections.push(Section {
                    label,
                    pages: vec![page.clone()],
                }),
            }
        }
        sections
    }
}

/// One labelled run of the docs sidebar.
#[derive(Debug, Clone)]
pub struct Section {
    /// Empty for the leading group, which is rendered without a heading.
    pub label: String,
    pub pages: Vec<Page>,
}

/// The whole site's content.
#[derive(Debug, Clone)]
pub struct Library {
    pub home: Home,
    pub book: Collection,
    pub docs: Collection,
}

/// The landing page.
///
/// Its structure is front matter rather than prose because a landing page is
/// composed, not written: the hero, the woven code panels and the feature grid
/// are laid out by the template and only their text belongs to the author. The
/// markdown body under the front matter becomes the closing section, which is
/// the one part of the page that really is prose.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Home {
    pub title: String,
    pub eyebrow: String,
    pub lede: String,
    /// `lede` rendered inline; filled in after deserializing.
    #[serde(skip)]
    pub lede_html: String,
    pub actions: Vec<Action>,
    pub weave: Weave,
    #[serde(default, rename = "feature")]
    pub features: Vec<Feature>,
    #[serde(default, rename = "closing")]
    pub closing_title: String,
    /// Rendered from the markdown body, so it is filled in after deserializing.
    #[serde(skip)]
    pub closing: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub label: String,
    pub href: String,
    #[serde(default)]
    pub primary: bool,
}

/// The hero's three panels: the two files an author writes, and what they
/// compile to. The site's one argument, made in the space of a screen.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weave {
    pub rs_name: String,
    pub rs: String,
    pub dmk_name: String,
    pub dmk: String,
    pub out_name: String,
    pub out: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    pub title: String,
    pub body: String,
    /// `body` rendered inline; filled in after deserializing.
    #[serde(skip)]
    pub body_html: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default = "default_lang")]
    pub lang: String,
}

fn default_lang() -> String {
    "dmk".into()
}

/// What a `.md` file's front matter may say.
///
/// `deny_unknown_fields` on purpose: a mistyped key would otherwise be dropped
/// in silence, and the first anyone would know of it is a page that renders
/// without its summary.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Front {
    title: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    section: Option<String>,
}

/// A collection's landing page — `content/book.md`, `content/docs.md`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionFront {
    title: String,
    lede: String,
}

/// Reads `content/` and renders every document in it.
pub fn load(root: &Path, urls: &Urls, highlighter: &Highlighter) -> Result<Library, Error> {
    Ok(Library {
        home: home(&root.join("home.md"), urls, highlighter)?,
        book: collection(root, Kind::Book, urls, highlighter)?,
        docs: collection(root, Kind::Docs, urls, highlighter)?,
    })
}

fn home(path: &Path, urls: &Urls, highlighter: &Highlighter) -> Result<Home, Error> {
    let source = read(path)?;
    let (front, body) = split(&source, path)?;
    let mut home: Home =
        toml::from_str(front).map_err(|error| format!("{}: {error}", path.display()))?;

    // The front matter writes links as the author thinks of them — `/book/` —
    // and the deploy decides what that means. Every other href on the site is
    // built through `Urls`; these are the only ones that arrive as text, so this
    // is where they join the rest.
    for action in &mut home.actions {
        if action.href.starts_with('/') {
            action.href = urls.to(&action.href);
        }
    }

    home.lede_html = markdown::inline(&home.lede, urls, highlighter);
    for feature in &mut home.features {
        feature.body_html = markdown::inline(&feature.body, urls, highlighter);
    }

    home.closing = markdown::render(body, urls, highlighter).html;
    Ok(home)
}

fn collection(
    root: &Path,
    kind: Kind,
    urls: &Urls,
    highlighter: &Highlighter,
) -> Result<Collection, Error> {
    let index_path = root.join(format!("{}.md", kind.dir()));
    let source = read(&index_path)?;
    let (front, body) = split(&source, &index_path)?;
    let index: CollectionFront =
        toml::from_str(front).map_err(|error| format!("{}: {error}", index_path.display()))?;

    let dir = root.join(kind.dir());
    let mut pages = Vec::new();
    for path in ordered_markdown(&dir)? {
        pages.push(page(&path, kind, urls, highlighter)?);
    }

    Ok(Collection {
        kind,
        title: index.title,
        lede_html: markdown::inline(&index.lede, urls, highlighter),
        lede: index.lede,
        intro: markdown::render(body, urls, highlighter).html,
        href: urls.to(&format!("/{}/", kind.dir())),
        pages,
    })
}

fn page(path: &Path, kind: Kind, urls: &Urls, highlighter: &Highlighter) -> Result<Page, Error> {
    let source = read(path)?;
    let (front, body) = split(&source, path)?;
    let front: Front =
        toml::from_str(front).map_err(|error| format!("{}: {error}", path.display()))?;

    let slug = slug(path);
    let rendered = markdown::render(body, urls, highlighter);

    Ok(Page {
        href: urls.to(&format!("/{}/{slug}/", kind.dir())),
        slug,
        title: front.title,
        summary_html: markdown::inline(&front.summary, urls, highlighter),
        summary: front.summary,
        section: front.section,
        body: rendered.html,
        headings: rendered.headings,
    })
}

/// The `.md` files in a directory, sorted by filename.
fn ordered_markdown(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let entries = fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    paths.sort();

    if paths.is_empty() {
        return Err(format!("no markdown files in {}", dir.display()));
    }
    Ok(paths)
}

/// `03-templates.md` → `templates`.
///
/// The numeric prefix orders the file and then gets out of the way, so
/// renumbering a chapter does not change its URL.
fn slug(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();

    match stem.split_once('-') {
        Some((prefix, rest)) if prefix.chars().all(|c| c.is_ascii_digit()) => rest.to_string(),
        _ => stem.to_string(),
    }
}

fn read(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))
}

/// Splits `+++` front matter from the markdown body.
///
/// `+++` rather than `---`, because `---` is also a markdown horizontal rule and
/// a setext heading underline — a body that opens with either would be eaten by
/// a scanner looking for the closing fence.
fn split<'a>(source: &'a str, path: &Path) -> Result<(&'a str, &'a str), Error> {
    let missing = || {
        format!(
            "{}: expected TOML front matter fenced by `+++` at the top of the file",
            path.display()
        )
    };

    let rest = source.strip_prefix("+++").ok_or_else(missing)?;
    let (front, body) = rest.split_once("\n+++").ok_or_else(missing)?;
    Ok((front, body.trim_start_matches(['\r', '\n'])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_numeric_prefix_orders_a_file_without_reaching_the_url() {
        assert_eq!(slug(Path::new("content/book/03-templates.md")), "templates");
        assert_eq!(slug(Path::new("content/docs/syntax.md")), "syntax");
        // Not a prefix: the digits have to be the whole first segment.
        assert_eq!(slug(Path::new("content/book/v2-notes.md")), "v2-notes");
    }

    #[test]
    fn front_matter_splits_off_the_body() {
        let (front, body) = split("+++\ntitle = \"A\"\n+++\n# Heading\n", Path::new("x")).unwrap();
        assert_eq!(front.trim(), "title = \"A\"");
        assert_eq!(body, "# Heading\n");
    }

    /// The reason the fence is `+++`: a body may legitimately start with `---`.
    #[test]
    fn a_body_may_open_with_a_horizontal_rule() {
        let (_, body) = split("+++\ntitle = \"A\"\n+++\n---\ntext\n", Path::new("x")).unwrap();
        assert_eq!(body, "---\ntext\n");
    }

    #[test]
    fn a_file_without_front_matter_is_an_error() {
        assert!(split("# Just markdown\n", Path::new("x")).is_err());
    }
}
