//! The Damask website generator.
//!
//! Reads `content/` — markdown with TOML front matter — renders it through the
//! components in `view/`, and writes a static tree of HTML.
//!
//! ```sh
//! cargo run -p damask-site -- --out dist --base /damask
//! ```
//!
//! There is no incremental mode and no watcher. The whole site is a few dozen
//! documents; rebuilding all of them takes less time than deciding which ones
//! changed, and a generator that can only be wrong about staleness is a
//! generator that cannot be wrong about it.

mod content;
mod highlight;
mod links;
mod markdown;
mod urls;
mod view;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use damask::Component;

use content::{Collection, Library};
use highlight::Highlighter;
use links::Targets;
use urls::Urls;
use view::chrome::{Chrome, Here};
use view::pages;
use view::ui::Step;

/// The one directory served verbatim, and the URL prefix it is served under.
/// Named once because the copy and the link check must agree about it.
const ASSETS: &str = "assets";

fn main() -> ExitCode {
    match run() {
        Ok(count) => {
            println!("wrote {count} pages");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("damask-site: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Where to read from, where to write to, and what path the site is served at.
struct Args {
    content: PathBuf,
    assets: PathBuf,
    out: PathBuf,
    base: String,
}

impl Args {
    /// Hand-parsed. Four options with obvious defaults do not need a CLI
    /// dependency, and the site is built by a script that passes the same flags
    /// every time.
    fn parse() -> Result<Self, String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut args = Args {
            content: root.join("content"),
            assets: root.join("assets"),
            out: root.join("dist"),
            base: String::new(),
        };

        let mut argv = std::env::args().skip(1);
        while let Some(flag) = argv.next() {
            let mut value = || argv.next().ok_or_else(|| format!("`{flag}` needs a value"));
            match flag.as_str() {
                "--content" => args.content = PathBuf::from(value()?),
                "--assets" => args.assets = PathBuf::from(value()?),
                "--out" => args.out = PathBuf::from(value()?),
                "--base" => args.base = value()?,
                other => return Err(format!("unknown option `{other}`")),
            }
        }
        Ok(args)
    }
}

fn run() -> Result<usize, String> {
    let args = Args::parse()?;
    let urls = Urls::new(&args.base);
    let highlighter = Highlighter::new();
    let library = content::load(&args.content, &urls, &highlighter)?;

    // Rebuilt rather than written over: a page removed from `content/` has to
    // disappear from the output too, and an overwrite would leave it behind to
    // be deployed forever.
    if args.out.exists() {
        fs::remove_dir_all(&args.out).map_err(|e| format!("clear {}: {e}", args.out.display()))?;
    }

    let pages = render(&library, &urls, &highlighter);

    for page in &pages {
        write(&args.out.join(&page.path), &page.html)?;
    }

    let assets = copy_tree(&args.assets, &args.out.join(ASSETS))?;
    check_links(&pages, &assets, &urls)?;

    // GitHub Pages runs Jekyll over the branch unless told not to, and Jekyll
    // drops files and directories whose names begin with an underscore.
    write(&args.out.join(".nojekyll"), "")?;

    Ok(pages.len())
}

/// Every page of the site, as an output path and its HTML.
///
/// Clean URLs, so each page is an `index.html` in a directory of its own — a
/// static host serves `/book/slots/` from `book/slots/index.html` with no
/// rewrite rules, which is the whole reason not to emit `slots.html`.
fn render(library: &Library, urls: &Urls, highlighter: &Highlighter) -> Vec<Rendered> {
    let mut pages = vec![Rendered {
        path: PathBuf::from("index.html"),
        href: urls.to("/"),
        anchors: Vec::new(),
        html: home(library, urls, highlighter),
    }];

    for collection in [&library.book, &library.docs] {
        let dir = PathBuf::from(collection.kind.dir());
        pages.push(Rendered {
            path: dir.join("index.html"),
            href: collection.href.clone(),
            anchors: Vec::new(),
            html: index(collection, urls),
        });

        for (position, page) in collection.pages.iter().enumerate() {
            pages.push(Rendered {
                path: dir.join(&page.slug).join("index.html"),
                href: page.href.clone(),
                anchors: page.headings.iter().map(|h| h.anchor.clone()).collect(),
                html: article(collection, position, urls),
            });
        }
    }

    pages
}

/// One written page, and what it offers other pages as a destination.
struct Rendered {
    path: PathBuf,
    href: String,
    anchors: Vec<String>,
    html: String,
}

/// Fails the build on a link that does not resolve.
///
/// After everything is rendered, because a link is only checkable once every
/// page it might point at is known — and reporting all of them at once is what
/// makes fixing a renamed page one edit rather than one build per link.
fn check_links(pages: &[Rendered], assets: &[String], urls: &Urls) -> Result<(), String> {
    let mut targets = Targets::default();
    for page in pages {
        targets.add_page(&page.href, page.anchors.iter().cloned());
    }
    for asset in assets {
        targets.add_asset(&urls.to(&format!("/{ASSETS}{asset}")));
    }

    let broken: Vec<String> = pages
        .iter()
        .flat_map(|page| links::check(&page.href, &page.html, &targets))
        .collect();

    if broken.is_empty() {
        Ok(())
    } else {
        Err(format!("broken links:\n  {}", broken.join("\n  ")))
    }
}

fn home(library: &Library, urls: &Urls, highlighter: &Highlighter) -> String {
    let home = &library.home;
    let weave = &home.weave;

    let features = home
        .features
        .iter()
        .map(|feature| pages::Feature {
            title: feature.title.clone(),
            body_html: feature.body_html.clone(),
            code: feature
                .code
                .as_ref()
                .map(|code| highlighter.block(&feature.lang, code)),
        })
        .collect();

    pages::Home {
        chrome: Chrome::new(urls, Here::Home, &home.title, markdown::plain(&home.lede)),
        rs: highlighter.block("rust", &weave.rs),
        dmk: highlighter.block("dmk", &weave.dmk),
        out: highlighter.block("html", &weave.out),
        content: home.clone(),
        features,
        book_href: library.book.href.clone(),
        docs_href: library.docs.href.clone(),
    }
    .render()
}

fn index(collection: &Collection, urls: &Urls) -> String {
    pages::Index {
        chrome: Chrome::new(
            urls,
            collection.kind.into(),
            &collection.title,
            markdown::plain(&collection.lede),
        ),
        collection: collection.clone(),
    }
    .render()
}

fn article(collection: &Collection, position: usize, urls: &Urls) -> String {
    let page = &collection.pages[position];

    // The pager's neighbours come from the flat page order, not from the grouped
    // sidebar: "next" means the next document, and a section boundary is not a
    // reason to stop reading.
    let previous = Step::of(
        position
            .checked_sub(1)
            .and_then(|i| collection.pages.get(i)),
    );
    let next = Step::of(collection.pages.get(position + 1));

    // A page without a summary still needs a meta description, and the
    // collection's lede is the truest thing available about it.
    let description = markdown::plain(if page.summary.is_empty() {
        &collection.lede
    } else {
        &page.summary
    });

    pages::Article {
        chrome: Chrome::new(urls, collection.kind.into(), &page.title, description),
        page: page.clone(),
        sections: collection.sections(),
        collection_title: collection.title.clone(),
        collection_href: collection.href.clone(),
        kind: collection.kind,
        previous,
        next,
    }
    .render()
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Copies `from` into `to`, returning each copied file as a root-relative href.
///
/// The hrefs are what the link check needs; producing them here rather than by
/// walking the output afterwards means the list cannot disagree with what was
/// actually written.
fn copy_tree(from: &Path, to: &Path) -> Result<Vec<String>, String> {
    fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    let mut copied = Vec::new();

    let entries = fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read an entry of {}: {e}", from.display()))?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        let name = entry.file_name().to_string_lossy().into_owned();

        if source.is_dir() {
            for nested in copy_tree(&source, &target)? {
                copied.push(format!("/{name}{nested}"));
            }
        } else {
            fs::copy(&source, &target)
                .map_err(|e| format!("copy {} → {}: {e}", source.display(), target.display()))?;
            copied.push(format!("/{name}"));
        }
    }
    Ok(copied)
}
