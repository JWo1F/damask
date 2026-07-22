//! What every page needs around its own content.
//!
//! One value threaded through the layouts rather than a dozen props on each
//! page struct, for the same reason the router's `Shell` exists: it is assembled
//! once and passed on untouched, so a page rendering a chapter has no reason to
//! name the fields of the site header.

use crate::content::Kind;
use crate::urls::Urls;

/// Which top-level surface the reader is on. Drives the header's active state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Here {
    Home,
    Book,
    Docs,
}

impl From<Kind> for Here {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Book => Here::Book,
            Kind::Docs => Here::Docs,
        }
    }
}

/// A page offered when the search dialog opens with nothing typed.
///
/// Built from the real content rather than written into the template, so a
/// suggestion's label is the page's actual title and cannot drift when a
/// chapter is renamed. The hrefs go through the link checker like any other.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub title: String,
    pub href: String,
    /// "Book" or "Reference".
    pub kind: &'static str,
    /// The page's summary. Carried so a suggested row is the same three lines a
    /// search result is — the idle state differs from a result list in what it
    /// lists, not in how a row looks.
    pub summary: String,
}

/// One entry in the site header.
#[derive(Debug, Clone)]
pub struct NavItem {
    pub label: &'static str,
    pub href: String,
    pub here: Here,
}

#[derive(Debug, Clone)]
pub struct Chrome {
    /// The page's own title, before the site name is appended.
    pub title: String,
    pub description: String,
    pub here: Here,
    pub nav: Vec<NavItem>,
    /// Where to send a reader who opened search with nothing in mind.
    pub suggestions: Vec<Suggestion>,
    pub urls: Urls,
}

impl Chrome {
    pub const NAME: &'static str = "Damask";
    pub const REPO: &'static str = "https://github.com/jwo1f/damask";

    pub fn new(
        urls: &Urls,
        here: Here,
        title: impl Into<String>,
        description: impl Into<String>,
        suggestions: Vec<Suggestion>,
    ) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            here,
            suggestions,
            nav: vec![
                NavItem {
                    label: "Book",
                    href: urls.to("/book/"),
                    here: Here::Book,
                },
                NavItem {
                    label: "Reference",
                    href: urls.to("/docs/"),
                    here: Here::Docs,
                },
            ],
            urls: urls.clone(),
        }
    }

    /// The document title. The home page is already named by its own title, so
    /// appending the site name there would say "Damask · Damask".
    pub fn document_title(&self) -> String {
        if self.here == Here::Home {
            self.title.clone()
        } else {
            format!("{} · {}", self.title, Self::NAME)
        }
    }

    /// Which half of the site this page belongs to, or nothing for the home
    /// page — which the header already offers and is not worth a slot in the
    /// reader's history.
    pub fn kind_label(&self) -> Option<&'static str> {
        match self.here {
            Here::Home => None,
            Here::Book => Some("Book"),
            Here::Docs => Some("Reference"),
        }
    }

    pub fn asset(&self, path: &str) -> String {
        self.urls.to(path)
    }

    pub fn home(&self) -> String {
        self.urls.to("/")
    }
}
