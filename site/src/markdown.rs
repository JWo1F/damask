//! Markdown → HTML, with the two things a documentation site needs from it:
//! stable heading anchors and syntax-highlighted code.
//!
//! Code blocks are highlighted here rather than in the browser. A docs page is
//! mostly code, and shipping a highlighter to colour markup the generator
//! already had in memory is work done twice — once at build time it would be
//! done anyway, once again on every reader's machine.

use std::sync::Arc;

use comrak::nodes::{NodeHtmlBlock, NodeValue};
use comrak::{Anchorizer, Arena, Options, parse_document};

use crate::highlight::Highlighter;
use crate::urls::Urls;

/// A heading deep enough to be worth a table-of-contents entry.
#[derive(Debug, Clone)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    /// The `id` Comrak wrote on the heading, so a link here actually lands.
    pub anchor: String,
}

/// One document's markdown, rendered.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub html: String,
    pub headings: Vec<Heading>,
}

/// Renders a markdown body.
///
/// The heading list is collected from the same parse that produced the HTML,
/// and anchorized with the same algorithm in the same order — which is what
/// makes the two agree. Anchor uniquing is order-dependent (a second "Options"
/// becomes `options-1`), so a table of contents built from a separate pass over
/// the source would silently drift from the document it points into.
pub fn render(source: &str, urls: &Urls, highlighter: &Highlighter) -> Rendered {
    let arena = Arena::new();
    let options = options(urls);
    let root = parse_document(&arena, source, &options);

    let mut headings = Vec::new();
    let mut anchorizer = Anchorizer::new();

    for node in root.descendants() {
        // The borrow is taken, read from, and dropped before anything else
        // touches the node. Both arms below need the node again — `collect_text`
        // walks it, and the code-block arm writes to it — and a borrow still
        // held at that point is a panic, not a compile error.
        let interest = {
            let data = node.data.borrow();
            match &data.value {
                NodeValue::Heading(heading) => Interest::Heading(heading.level),
                NodeValue::CodeBlock(block) => {
                    Interest::Code(block.info.clone(), block.literal.clone())
                }
                _ => Interest::None,
            }
        };

        match interest {
            Interest::Heading(level) => {
                let text = node.collect_text();
                let anchor = anchorizer.anchorize(&text);
                headings.push(Heading {
                    level,
                    text,
                    anchor,
                });
            }

            // Swapping the code block for pre-rendered markup is what lets the
            // highlighter run at build time without a Comrak plugin: the node is
            // already HTML by the time the formatter walks the tree.
            Interest::Code(info, literal) => {
                let language = info.split_whitespace().next().unwrap_or_default();
                let html = highlighter.block(language, &literal);
                node.data.borrow_mut().value = NodeValue::HtmlBlock(NodeHtmlBlock {
                    block_type: 0,
                    literal: html,
                });
            }

            Interest::None => {}
        }
    }

    let mut html = String::new();
    comrak::format_html(root, &options, &mut html).expect("format a markdown document as HTML");

    Rendered { html, headings }
}

/// Renders a one-line string as **inline** markdown: no wrapping paragraph.
///
/// Front matter carries prose too — a lede, a summary, a feature's body — and
/// an author writing `` `render` `` there means the same thing they mean in a
/// document. Rendering it as a block would wrap each in a `<p>` the template
/// already provides, so the paragraph is unwrapped here rather than styled
/// around at every call site.
///
/// Multi-paragraph input keeps its later paragraphs intact, which is the honest
/// behaviour: a field that turns out to hold two paragraphs should render as
/// two, not silently lose one.
pub fn inline(source: &str, urls: &Urls, highlighter: &Highlighter) -> String {
    let html = render(source, urls, highlighter).html;
    let trimmed = html.trim();

    match trimmed
        .strip_prefix("<p>")
        .and_then(|rest| rest.strip_suffix("</p>"))
    {
        // Only when the whole string was one paragraph: a `</p>` in the middle
        // means there is a second one, and stripping the outer tags would splice
        // two paragraphs into one line.
        Some(inner) if !inner.contains("</p>") => inner.to_string(),
        _ => trimmed.to_string(),
    }
}

/// Strips the inline markers that would otherwise show up verbatim in a `<meta>`
/// description, where there is no renderer to interpret them.
///
/// Deliberately not a markdown parser. A description is one sentence of prose,
/// the only markers that appear in one are code ticks and emphasis, and running
/// the real renderer here would produce HTML — which is exactly what a meta
/// attribute must not contain.
pub fn plain(source: &str) -> String {
    source.replace(['`', '*', '_'], "")
}

/// What a node turned out to be, carried out of the borrow that read it.
enum Interest {
    Heading(u8),
    /// The fence's info string and its body.
    Code(String, String),
    None,
}

fn options<'a>(urls: &Urls) -> Options<'a> {
    let mut options = Options::default();

    // Prose links and images are written root-relative — `/docs/slots/` — because
    // that is how an author thinks about the site. Where it is actually mounted
    // is a deploy-time fact, and rewriting here is what keeps `Urls` the single
    // place that knows it: an author never writes the base, and cannot forget to.
    let rewrite = {
        let urls = urls.clone();
        move |url: &str| {
            if url.starts_with('/') {
                urls.to(url)
            } else {
                url.to_string()
            }
        }
    };
    options.extension.link_url_rewriter = Some(Arc::new(rewrite.clone()));
    options.extension.image_url_rewriter = Some(Arc::new(rewrite));

    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.description_lists = true;
    // GitHub-style `> [!NOTE]` callouts, so a caveat in the prose is a caveat in
    // the page without an HTML escape hatch in the markdown.
    options.extension.alerts = true;
    // `Some("")` is what turns heading ids on at all; the prefix itself is empty
    // because these pages have one document each and nothing to collide with.
    options.extension.header_id_prefix = Some(String::new());

    options.parse.smart = true;

    // The content tree is part of this repository, so raw HTML in it is markup
    // the site's authors wrote — the same trust level as a `.dmk`. Nothing
    // user-submitted reaches this renderer; if that ever changes, this is the
    // line that has to change with it.
    options.render.r#unsafe = true;
    options.render.github_pre_lang = true;

    options
}
