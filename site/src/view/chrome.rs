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
    ) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            here,
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

    pub fn asset(&self, path: &str) -> String {
        self.urls.to(path)
    }

    pub fn home(&self) -> String {
        self.urls.to("/")
    }
}
