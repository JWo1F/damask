//! The overlay handed to rust-analyzer for a component's template.
//!
//! rust-analyzer only understands files that belong to the crate graph, and a
//! `.dmk` template is not one. The trick is to never show it a `.dmk` at all:
//! instead we take the component's *paired* `.rs` — already a crate module — and
//! append a synthetic method whose body is the lowered template:
//!
//! ```ignore
//! // ...the original greeting.rs, unchanged...
//!
//! impl Greeting {
//!     fn __damask_check(&self, __damask: &mut dyn ::damask::Renderer, __damask_slots: ::damask::Slots<'_>) {
//!         // lowered body of greeting.dmk
//!         __damask.write_escaped(&(self.name));
//!     }
//! }
//! ```
//!
//! Because this is literally the component's own module, `self`, sibling
//! components, `std`, and dependencies all resolve. We send it to rust-analyzer
//! as an in-memory overlay (never touching disk) and keep a byte-offset map from
//! the `.dmk` source to the appended region, so a request at a template position
//! can be forwarded to the equivalent overlay position and results mapped back.

use damask_template::{Span, Template, lower_mapped};

/// The synthetic method name appended to the paired module. Chosen to be
/// unlikely to collide with anything a component author would write.
const CHECK_FN: &str = "__damask_check";

/// A verbatim, equal-length correspondence between a `.dmk` source range and a
/// range in the overlay `.rs`.
#[derive(Debug, Clone, Copy)]
struct OverlayMapping {
    source: Span,
    overlay: Span,
}

/// A component's paired `.rs` with the lowered template appended as a synthetic
/// method, plus the position map tying the template back to it.
pub struct VirtualFile {
    /// The overlay `.rs` text to send to rust-analyzer.
    pub text: String,
    /// Byte offset in [`text`](Self::text) where the lowered body begins.
    body_base: usize,
    /// Source↔overlay correspondences, in overlay order.
    mappings: Vec<OverlayMapping>,
}

impl VirtualFile {
    /// Build the overlay for `struct_name`, whose paired file is `rs_src` and
    /// whose template parses to `template`. `Err` if the template fails to lower.
    ///
    /// Note: the appended `impl` does not yet replicate the struct's generic
    /// parameters, so generic components are not fully supported here.
    pub fn build(rs_src: &str, struct_name: &str, template: &Template) -> Result<Self, String> {
        let (body, map) = lower_mapped(template)?;

        let mut text = String::with_capacity(rs_src.len() + body.len() + 160);
        text.push_str(rs_src);
        // A blank line separates the synthetic code from the user's; the
        // `#[allow]` silences warnings the generated method would otherwise
        // raise (its `self`/`__damask` uses, redundant braces, and so on).
        text.push_str(
            "\n\n#[allow(dead_code, unused, unused_braces, unused_parens, clippy::all)]\nimpl ",
        );
        text.push_str(struct_name);
        text.push_str("\n{\n    fn ");
        text.push_str(CHECK_FN);
        text.push_str(
            "(&self, __damask: &mut dyn ::damask::Renderer, __damask_slots: ::damask::Slots<'_>) ",
        );
        let body_base = text.len();
        text.push_str(&body);
        text.push_str("\n}\n");

        let mappings = map
            .mappings
            .iter()
            .map(|m| OverlayMapping {
                source: m.source,
                overlay: Span::new(m.generated.start + body_base, m.generated.end + body_base),
            })
            .collect();

        Ok(VirtualFile {
            text,
            body_base,
            mappings,
        })
    }

    /// Translate a byte offset in the `.dmk` source to the overlay, if it lands
    /// inside a mapped template fragment.
    pub fn source_to_overlay(&self, offset: usize) -> Option<usize> {
        let m = self.mappings.iter().find(|m| contains(m.source, offset))?;
        Some(m.overlay.start + (offset - m.source.start))
    }

    /// Like [`source_to_overlay`](Self::source_to_overlay), but also maps a
    /// cursor at the exclusive end of a fragment — e.g. just after `self.`, the
    /// position completion is requested at. Falls back to mapping the byte
    /// before the cursor and stepping one past it in the overlay.
    pub fn source_to_overlay_boundary(&self, offset: usize) -> Option<usize> {
        self.source_to_overlay(offset).or_else(|| {
            self.source_to_overlay(offset.checked_sub(1)?)
                .map(|o| o + 1)
        })
    }

    /// Translate a byte offset in the overlay back to the `.dmk` source, if it
    /// lands inside a mapped fragment of the appended body.
    pub fn overlay_to_source(&self, offset: usize) -> Option<usize> {
        let m = self.mappings.iter().find(|m| contains(m.overlay, offset))?;
        Some(m.source.start + (offset - m.overlay.start))
    }

    /// Whether an overlay offset lies within the appended synthetic body (as
    /// opposed to the user's original file). Used to filter rust-analyzer
    /// diagnostics down to the template's contribution.
    pub fn in_body(&self, offset: usize) -> bool {
        offset >= self.body_base
    }
}

/// Half-open containment: `[start, end)`.
fn contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests {
    use super::*;

    const RS: &str = "pub struct Greeting {\n    pub name: String,\n}\n";

    fn build(damask: &str) -> (VirtualFile, String) {
        let template = damask_template::parse(damask).unwrap();
        let vf = VirtualFile::build(RS, "Greeting", &template).unwrap();
        (vf, damask.to_string())
    }

    #[test]
    fn overlay_wraps_body_in_struct_impl() {
        let (vf, _) = build("Hello {self.name}!");
        assert!(vf.text.starts_with(RS), "original file preserved verbatim");
        assert!(vf.text.contains("impl Greeting"));
        assert!(vf.text.contains(
            "fn __damask_check(&self, __damask: &mut dyn ::damask::Renderer, __damask_slots: ::damask::Slots<'_>)"
        ));
        assert!(vf.text.contains("self.name"));
    }

    #[test]
    fn source_offset_round_trips_to_overlay_and_back() {
        let damask = "Hello {self.name}!";
        let (vf, _) = build(damask);
        // Offset of "name" within the template.
        let name_at = damask.find("name").unwrap();
        let ov = vf.source_to_overlay(name_at).expect("mapped");
        assert_eq!(&vf.text[ov..ov + 4], "name");
        assert_eq!(vf.overlay_to_source(ov), Some(name_at));
        assert!(vf.in_body(ov));
    }

    #[test]
    fn text_outside_fragments_is_unmapped() {
        let damask = "Hello {self.name}!";
        let (vf, _) = build(damask);
        // The literal "Hello " is not a Rust fragment — no mapping.
        assert_eq!(vf.source_to_overlay(0), None);
    }

    /// Hover and go-to-definition work by mapping a source offset into the
    /// overlay, so every Rust position in a class value has to be mapped —
    /// including the ones with no `{ … }` around them, which is every entry of
    /// a list.
    #[test]
    fn class_values_are_mapped_for_hover() {
        let damask = r#"<div class=[self.extra, { "on": self.ok }] class:off={!self.ok}></div>"#;
        let (vf, _) = build(damask);
        for needle in ["self.extra", "self.ok }", "!self.ok"] {
            let at = damask.find(needle).unwrap();
            let ov = vf
                .source_to_overlay(at)
                .unwrap_or_else(|| panic!("`{needle}` is unmapped, so nothing can hover it"));
            let want = needle.trim_end_matches(" }");
            assert_eq!(&vf.text[ov..ov + want.len()], want);
            assert_eq!(vf.overlay_to_source(ov), Some(at));
        }
    }

    /// Every mapped source byte must translate to an overlay byte that holds the
    /// same character, and translate back to itself.
    #[test]
    fn all_mapped_bytes_round_trip_verbatim() {
        let damask = concat!(
            "{#if self.ok}{self.a}{:else}{self.b}{/if}",
            "{#each &self.items as item, i}{item}{i}{/each}",
            r#"<a href={self.url}>x</a>"#,
        );
        let (vf, src) = build(damask);
        // Walk every mapping and every offset inside its source span.
        let src_bytes = src.as_bytes();
        for m in &vf.mappings {
            // The index is the value under test — it's fed to the translation
            // functions, not just used to reach into `src_bytes`.
            #[allow(clippy::needless_range_loop)]
            for off in m.source.start..m.source.end {
                let ov = vf.source_to_overlay(off).expect("mapped offset");
                assert_eq!(
                    vf.text.as_bytes()[ov],
                    src_bytes[off],
                    "byte mismatch at source {off}",
                );
                assert_eq!(vf.overlay_to_source(ov), Some(off));
            }
        }
    }
}
