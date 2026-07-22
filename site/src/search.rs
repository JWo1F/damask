//! The search index: an inverted index, serialized as a binary asset.
//!
//! A documentation site whose reference is "arranged for lookup" needs
//! something to look up with. The corpus is a few thousand words — small enough
//! to ship whole — so the search runs in the browser and there is no service to
//! keep alive.
//!
//! # Why binary rather than JSON
//!
//! A JSON corpus costs a `JSON.parse` over every byte before the first
//! keystroke can be answered, and then the browser holds a graph of objects and
//! strings. This format is laid out so the client does **no** preprocessing:
//! every numeric section is a run of little-endian `u32`s at a 4-byte-aligned
//! offset, so the reader wraps the `ArrayBuffer` in `Uint32Array` views and is
//! ready. Only the handful of strings a result actually shows are ever decoded.
//!
//! # Shape
//!
//! It is a `tsvector` turned inside out: rather than each document carrying its
//! lexemes, each lexeme carries its documents. A query intersects postings lists
//! instead of scanning documents, so cost tracks the rarest term in the query
//! rather than the size of the corpus.
//!
//! A *document* is a **section**, not a page. Someone searching `has_default`
//! wants the paragraph that defines it, not the top of a page that mentions it
//! somewhere, so every `<h2>`/`<h3>` becomes its own document carrying its own
//! anchor.
//!
//! ```text
//! magic "DMKS" | version | counts | section offsets      (64-byte header)
//! docs               doc_count × 6 u32
//! term_offsets       (term_count + 1) u32   → into term_blob
//! term_blob          UTF-8, terms sorted    → binary-searchable, prefixes free
//! posting_offsets    (term_count + 1) u32   → into postings, in entries
//! postings           posting_count × 2 u32  → (doc, fields<<16 | tf)
//! string_offsets     (string_count + 1) u32 → into string_blob
//! string_blob        UTF-8
//! ```
//!
//! Terms are sorted, so a prefix query is a binary search for the lower bound
//! and a walk while the prefix still matches — which is what makes
//! search-as-you-type work without storing a trie.
//!
//! # What this deliberately does not do
//!
//! No stemming. A stemmer has to agree exactly between the Rust that writes the
//! index and the JavaScript that reads it, and a silent disagreement is a query
//! that returns nothing for no visible reason. Prefix matching covers most of
//! what stemming would buy here — `slot` finds `slots` — at none of that risk.

use crate::content::{Collection, Library};

/// Field a term occurred in, as a bit so one posting can carry several.
const FIELD_TITLE: u32 = 1;
const FIELD_HEADING: u32 = 2;
const FIELD_BODY: u32 = 4;

/// How much of a section's text to keep for the result list.
const PREVIEW: usize = 180;

const MAGIC: &[u8; 4] = b"DMKS";
const VERSION: u32 = 1;
const HEADER: usize = 64;

/// One searchable section, before it is turned into postings.
struct Doc {
    page: String,
    heading: String,
    href: String,
    preview: String,
    /// 0 = book, 1 = reference.
    kind: u32,
    /// Term occurrences, already tokenized, tagged with the field they came
    /// from. Length in body terms is what BM25 normalizes against.
    terms: Vec<(String, u32)>,
}

/// Builds the binary index.
pub fn index(library: &Library) -> Vec<u8> {
    let mut docs = Vec::new();
    for (collection, kind) in [(&library.book, 0), (&library.docs, 1)] {
        collect(collection, kind, &mut docs);
    }

    serialize(&docs)
}

fn collect(collection: &Collection, kind: u32, out: &mut Vec<Doc>) {
    for page in &collection.pages {
        for section in sections(&page.body) {
            if section.full.is_empty() && section.heading.is_empty() {
                continue;
            }

            let lead = section.anchor.is_empty();
            let href = if lead {
                page.href.clone()
            } else {
                format!("{}#{}", page.href, section.anchor)
            };
            // A page's own summary rides along with its lead section, so the
            // page is findable by its description without a document of its own.
            let full = if lead {
                format!("{} {}", page.summary, section.full)
            } else {
                section.full
            };
            let prose = if lead {
                format!("{} {}", page.summary, section.prose)
            } else {
                section.prose
            };

            let mut terms = Vec::new();
            for term in tokenize(&page.title) {
                terms.push((term, FIELD_TITLE));
            }
            for term in tokenize(&section.heading) {
                terms.push((term, FIELD_HEADING));
            }
            for term in tokenize(&full) {
                terms.push((term, FIELD_BODY));
            }

            out.push(Doc {
                page: page.title.clone(),
                heading: section.heading,
                href,
                // The preview shows prose; the index holds the code too.
                preview: preview(prose.trim()),
                kind,
                terms,
            });
        }
    }
}

/// Splits text into search terms.
///
/// An identifier yields **both** its parts and itself: `has_default` indexes as
/// `has`, `default`, *and* `has_default`. The parts alone are not enough —
/// `has` and `default` are each common enough that a search for the identifier
/// ranks any section discussing defaults above the one that defines it. The
/// whole identifier is rare, so it carries the query.
///
/// `HtmlRenderer` splits the same way, on the case boundary — without it the
/// term is only ever reachable by typing it from the first letter, and a search
/// for `renderer` misses every mention of it.
///
/// The client runs the identical rule; the two must not drift.
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in text.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        if chunk.is_empty() {
            continue;
        }
        let parts = split_identifier(chunk);
        if parts.len() > 1 {
            out.extend(parts.into_iter().map(|p| p.to_lowercase()));
        }
        out.push(chunk.to_lowercase());
    }
    out
}

/// Breaks an identifier at underscores and case boundaries.
///
/// `HTMLRenderer` breaks as `HTML` + `Renderer`: a run of capitals is one word
/// until the last of them, which belongs to the word starting there.
fn split_identifier(chunk: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    for word in chunk.split('_').filter(|w| !w.is_empty()) {
        let chars: Vec<char> = word.chars().collect();
        let mut start = 0;
        for i in 1..chars.len() {
            let boundary = (!chars[i - 1].is_uppercase() && chars[i].is_uppercase())
                || (chars[i - 1].is_uppercase()
                    && chars[i].is_uppercase()
                    && chars.get(i + 1).is_some_and(|c| c.is_lowercase()));
            if boundary {
                parts.push(&word[byte_at(word, start)..byte_at(word, i)]);
                start = i;
            }
        }
        parts.push(&word[byte_at(word, start)..]);
    }
    parts
}

fn byte_at(s: &str, chars: usize) -> usize {
    s.char_indices()
        .nth(chars)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn preview(text: &str) -> String {
    if text.chars().count() <= PREVIEW {
        return text.to_string();
    }
    let cut: String = text.chars().take(PREVIEW).collect();
    // Back off to a word boundary so the preview does not end mid-identifier.
    match cut.rfind(' ') {
        Some(at) => format!("{}…", &cut[..at]),
        None => format!("{cut}…"),
    }
}

// ---------------------------------------------------------------------------
// Serialization

/// Collects strings, handing back a stable id per distinct string.
#[derive(Default)]
struct Strings {
    blob: Vec<u8>,
    offsets: Vec<u32>,
    seen: std::collections::HashMap<String, u32>,
}

impl Strings {
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(id) = self.seen.get(s) {
            return *id;
        }
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        let id = (self.offsets.len() - 1) as u32;
        self.blob.extend_from_slice(s.as_bytes());
        self.offsets.push(self.blob.len() as u32);
        self.seen.insert(s.to_string(), id);
        id
    }
}

fn serialize(docs: &[Doc]) -> Vec<u8> {
    let mut strings = Strings::default();

    // Documents, and the per-document term frequencies.
    let mut doc_rows: Vec<[u32; 6]> = Vec::with_capacity(docs.len());
    // term → (doc, fields, tf)
    let mut by_term: std::collections::BTreeMap<&str, Vec<(u32, u32, u32)>> =
        std::collections::BTreeMap::new();

    for (id, doc) in docs.iter().enumerate() {
        let mut counts: std::collections::HashMap<&str, (u32, u32)> =
            std::collections::HashMap::new();
        let mut body_len = 0u32;
        for (term, field) in &doc.terms {
            if *field == FIELD_BODY {
                body_len += 1;
            }
            let entry = counts.entry(term.as_str()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 |= field;
        }
        for (term, (tf, fields)) in counts {
            by_term
                .entry(term)
                .or_default()
                .push((id as u32, fields, tf.min(0xffff)));
        }

        doc_rows.push([
            strings.intern(&doc.page),
            strings.intern(&doc.heading),
            strings.intern(&doc.href),
            strings.intern(&doc.preview),
            doc.kind,
            body_len.max(1),
        ]);
    }

    // Term dictionary, in sorted order — BTreeMap already guarantees it, and the
    // client's binary search depends on it.
    let mut term_blob: Vec<u8> = Vec::new();
    let mut term_offsets: Vec<u32> = vec![0];
    let mut posting_offsets: Vec<u32> = vec![0];
    let mut postings: Vec<u32> = Vec::new();

    for (term, mut entries) in by_term {
        term_blob.extend_from_slice(term.as_bytes());
        term_offsets.push(term_blob.len() as u32);

        entries.sort_by_key(|(doc, _, _)| *doc);
        for (doc, fields, tf) in entries {
            postings.push(doc);
            postings.push((fields << 16) | tf);
        }
        posting_offsets.push((postings.len() / 2) as u32);
    }

    let term_count = term_offsets.len() - 1;
    let string_count = strings.offsets.len().saturating_sub(1);
    let avg_len = if doc_rows.is_empty() {
        1
    } else {
        (doc_rows.iter().map(|r| r[5] as u64).sum::<u64>() / doc_rows.len() as u64).max(1) as u32
    };

    // Lay the sections out, padding each blob so the next `u32` run starts
    // aligned — an unaligned `Uint32Array` view is a runtime error in the
    // browser, not a slow path.
    let mut out = vec![0u8; HEADER];
    let off_docs = push_u32s(&mut out, doc_rows.iter().flatten().copied());
    let off_term_offsets = push_u32s(&mut out, term_offsets.iter().copied());
    let off_term_blob = push_bytes(&mut out, &term_blob);
    let off_posting_offsets = push_u32s(&mut out, posting_offsets.iter().copied());
    let off_postings = push_u32s(&mut out, postings.iter().copied());
    let off_string_offsets = push_u32s(&mut out, strings.offsets.iter().copied());
    let off_string_blob = push_bytes(&mut out, &strings.blob);

    out[0..4].copy_from_slice(MAGIC);
    let header = [
        VERSION,
        doc_rows.len() as u32,
        term_count as u32,
        string_count as u32,
        (postings.len() / 2) as u32,
        avg_len,
        off_docs,
        off_term_offsets,
        off_term_blob,
        off_posting_offsets,
        off_postings,
        off_string_offsets,
        off_string_blob,
    ];
    for (i, value) in header.iter().enumerate() {
        let at = 4 + i * 4;
        out[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    out
}

fn push_u32s(out: &mut Vec<u8>, values: impl Iterator<Item = u32>) -> u32 {
    align(out);
    let at = out.len() as u32;
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    at
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> u32 {
    align(out);
    let at = out.len() as u32;
    out.extend_from_slice(bytes);
    at
}

fn align(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

// ---------------------------------------------------------------------------
// Turning rendered HTML back into text

/// Splits a rendered body into `(heading, anchor, text)` at each `h2`/`h3`.
///
/// The lead section — everything above the first heading — comes back with both
/// its heading and its anchor empty, which is what makes it the page's own
/// document rather than a section of it.
fn sections(html: &str) -> Vec<Section> {
    let mut out = Vec::new();
    let mut heading = String::new();
    let mut anchor = String::new();
    let mut rest = html;

    let flush = |heading: String, anchor: String, chunk: &str, out: &mut Vec<Section>| {
        let text = extract(chunk);
        out.push(Section {
            heading,
            anchor,
            full: text.full.trim().to_string(),
            prose: text.prose.trim().to_string(),
        });
    };

    while let Some(at) = find_heading(rest) {
        flush(heading, anchor, &rest[..at.start], &mut out);
        heading = at.title;
        anchor = at.anchor;
        rest = &rest[at.end..];
    }
    flush(heading, anchor, rest, &mut out);
    out
}

/// One `<h2>`/`<h3>` region of a page.
struct Section {
    heading: String,
    anchor: String,
    full: String,
    prose: String,
}

/// Where a heading sits, and what it says.
struct Found {
    start: usize,
    end: usize,
    title: String,
    anchor: String,
}

fn find_heading(html: &str) -> Option<Found> {
    // Comrak writes `<h2 id="…">`, and that id is what the table of contents and
    // the link checker already agree on — so a heading without one is not a
    // destination, and is skipped rather than guessed at.
    let mut from = 0;
    loop {
        let (level, at) = ["<h2 id=\"", "<h3 id=\""]
            .iter()
            .filter_map(|tag| html[from..].find(tag).map(|i| (*tag, from + i)))
            .min_by_key(|(_, i)| *i)?;

        let after_id = at + level.len();
        let quote_end = after_id + html[after_id..].find('"')?;
        let anchor = html[after_id..quote_end].to_string();
        let open_end = quote_end + html[quote_end..].find('>')? + 1;

        let close = if level.starts_with("<h2") {
            "</h2>"
        } else {
            "</h3>"
        };
        let Some(close_at) = html[open_end..].find(close).map(|i| open_end + i) else {
            from = open_end;
            continue;
        };

        return Some(Found {
            start: at,
            end: close_at + close.len(),
            title: extract(&html[open_end..close_at]).full.trim().to_string(),
            anchor,
        });
    }
}

/// Text pulled out of rendered markup, in two forms.
struct Text {
    /// Everything, code included — what gets indexed, so a reader can search
    /// for `render_with` and find the sample that uses it.
    full: String,
    /// Prose only — what the result preview shows. A preview made of flattened
    /// source reads as noise and tells a reader nothing about the section.
    prose: String,
}

/// Strips tags and decodes entities, collapsing whitespace.
///
/// Three rules earn their complexity:
///
/// * A tag is a word boundary, so `<code>a</code><code>b</code>` does not index
///   as `ab` — **except inside `<pre>`**, where the highlighter wraps every
///   token in its own span and that rule would turn `#[derive(Component)]` into
///   `# [ derive ( Component ) ]`. Code already carries its own whitespace.
/// * `<figcaption>` is skipped entirely. It holds the code block's rail — the
///   language label and the copy button — which is chrome, not content, and
///   indexing it would make a search for `rust` match every page with a Rust
///   sample on it.
/// * Code goes into `full` but not into `prose`.
///
/// Deliberately not a parser. This runs over markup this build just produced;
/// there is no untrusted input and no malformed case to be robust against.
fn extract(html: &str) -> Text {
    let mut out = Text {
        full: String::new(),
        prose: String::new(),
    };
    let mut pre = 0usize;
    let mut skip = 0usize;
    let mut chars = html.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        match c {
            '<' => {
                let end = html[i..].find('>').map(|j| i + j + 1).unwrap_or(html.len());
                let tag = &html[i..end];
                let closing = tag.starts_with("</");
                let name: String = tag[if closing { 2 } else { 1 }..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_ascii_lowercase();

                match name.as_str() {
                    "pre" if closing => pre = pre.saturating_sub(1),
                    "pre" => pre += 1,
                    "figcaption" if closing => skip = skip.saturating_sub(1),
                    "figcaption" => skip += 1,
                    _ => {}
                }

                if skip == 0 && pre == 0 {
                    push_space(&mut out.full);
                    push_space(&mut out.prose);
                }

                while chars.peek().is_some_and(|(j, _)| *j < end) {
                    chars.next();
                }
            }
            _ if skip > 0 => {}
            '&' => {
                let mut name = String::new();
                while let Some((_, n)) = chars.peek().copied() {
                    chars.next();
                    if n == ';' {
                        break;
                    }
                    name.push(n);
                    if name.len() > 8 {
                        break;
                    }
                }
                let text = match name.as_str() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "#39" | "apos" => "'",
                    "nbsp" => " ",
                    _ => "",
                };
                out.full.push_str(text);
                if pre == 0 {
                    out.prose.push_str(text);
                }
            }
            c if c.is_whitespace() => {
                push_space(&mut out.full);
                if pre == 0 {
                    push_space(&mut out.prose);
                }
            }
            c => {
                out.full.push(c);
                if pre == 0 {
                    out.prose.push(c);
                }
            }
        }
    }
    out
}

fn push_space(out: &mut String) {
    if !out.ends_with(' ') && !out.is_empty() {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_word_boundaries() {
        assert_eq!(
            extract("<p><code>a</code><code>b</code></p>").full.trim(),
            "a b"
        );
    }

    #[test]
    fn entities_are_decoded() {
        assert_eq!(
            extract("&lt;slot&gt; &amp; &#39;x&#39;").full,
            "<slot> & 'x'"
        );
    }

    /// The highlighter wraps every token in a span; without the `<pre>`
    /// exception each one would become a word boundary.
    #[test]
    fn code_keeps_its_own_spacing_and_stays_out_of_the_preview() {
        let html = "<p>before</p><figure><figcaption>Rust</figcaption><pre class=\"code\"><code>\
                    <span>#[</span><span>derive</span><span>(Component)]</span></code></pre></figure>";
        let text = extract(html);
        assert!(
            text.full.contains("#[derive(Component)]"),
            "code was pulled apart: {:?}",
            text.full
        );
        assert!(
            !text.full.contains("Rust"),
            "the rail leaked into the index"
        );
        assert_eq!(text.prose.trim(), "before", "code leaked into the preview");
    }

    #[test]
    fn a_body_splits_at_its_headings() {
        let html =
            "<p>lead</p><h2 id=\"one\">One</h2><p>first</p><h3 id=\"two\">Two</h3><p>second</p>";
        let out = sections(html);
        assert_eq!(out.len(), 3);
        fn row(s: &Section) -> (&str, &str, &str) {
            (s.heading.as_str(), s.anchor.as_str(), s.full.as_str())
        }
        assert_eq!(row(&out[0]), ("", "", "lead"));
        assert_eq!(row(&out[1]), ("One", "one", "first"));
        assert_eq!(row(&out[2]), ("Two", "two", "second"));
    }

    /// The client repeats this rule; if the two ever disagree, a query silently
    /// stops matching.
    #[test]
    fn identifiers_split_on_underscores() {
        assert_eq!(
            tokenize("slots.has_default()"),
            ["slots", "has", "default", "has_default"]
        );
        assert_eq!(
            tokenize("HtmlRenderer"),
            ["html", "renderer", "htmlrenderer"]
        );
        assert_eq!(
            tokenize("HTMLRenderer"),
            ["html", "renderer", "htmlrenderer"]
        );
        assert_eq!(tokenize("ClassList"), ["class", "list", "classlist"]);
        assert_eq!(tokenize("render_with"), ["render", "with", "render_with"]);
        assert_eq!(tokenize("Rust 1.88"), ["rust", "1", "88"]);
    }

    #[test]
    fn the_header_is_well_formed_and_sections_are_aligned() {
        let docs = vec![Doc {
            page: "Slots".into(),
            heading: "Placement".into(),
            href: "/docs/slots/#placement".into(),
            preview: "A component places…".into(),
            kind: 1,
            terms: vec![("slot".into(), FIELD_TITLE), ("place".into(), FIELD_BODY)],
        }];
        let bytes = serialize(&docs);

        assert_eq!(&bytes[0..4], MAGIC);
        let word = |i: usize| u32::from_le_bytes(bytes[4 + i * 4..8 + i * 4].try_into().unwrap());
        assert_eq!(word(0), VERSION);
        assert_eq!(word(1), 1, "one document");
        assert_eq!(word(2), 2, "two distinct terms");

        // Every section offset must be 4-aligned or the client cannot make a
        // Uint32Array over it.
        for i in 6..13 {
            assert!(word(i).is_multiple_of(4), "section {i} is unaligned");
        }
    }
}
