//! End-to-end behavior of the example components: escaping, composition,
//! control flow, and the custom-renderer seam.

use rsc::{Component, Renderer};
use rsc_showcase::button::Button;
use rsc_showcase::card::Card;
use rsc_showcase::custom_renderer::UpcaseRenderer;
use rsc_showcase::greeting::Greeting;
use rsc_showcase::list::List;
use rsc_showcase::panel::Panel;
use rsc_showcase::theme::Theme;

#[test]
fn escaped_interpolation() {
    let g = Greeting {
        name: "<Ada> & \"Co\"".into(),
    };
    assert_eq!(g.render(), "Hello &lt;Ada&gt; &amp; &quot;Co&quot;!");
}

#[test]
fn composition_same_buffer_and_string_path_agree() {
    // `Card` uses `<%+ self.button %>`; `Panel` uses `<%- self.button.render() %>`.
    // They wrap the button identically, so their output must match.
    let card = Card {
        button: Button { label: "OK".into() },
    };
    let panel = Panel {
        button: Button { label: "OK".into() },
    };
    assert_eq!(card.render(), "<section><button>OK</button></section>");
    assert_eq!(card.render(), panel.render());
}

#[test]
fn child_output_is_embedded_raw_not_double_escaped() {
    // Button escapes its label once; the parent embeds that output verbatim.
    let card = Card {
        button: Button {
            label: "<b>".into(),
        },
    };
    let out = card.render();
    assert_eq!(out, "<section><button>&lt;b&gt;</button></section>");
    // The escaping happened exactly once — no `&amp;lt;`.
    assert!(!out.contains("&amp;"));
}

#[test]
fn control_flow_spans_multiple_tags() {
    let list = List {
        items: vec!["a".into(), "b<c>".into()],
    };
    let out = list.render();
    assert!(out.starts_with("<ul>"));
    assert!(out.trim_end().ends_with("</ul>"));
    assert!(out.contains("<li>a</li>"));
    // Loop body escapes each item.
    assert!(out.contains("<li>b&lt;c&gt;</li>"));
}

#[test]
fn empty_loop_renders_wrapper_only() {
    let list = List { items: vec![] };
    let out = list.render();
    assert!(out.contains("<ul>"));
    assert!(!out.contains("<li>"));
}

#[test]
fn css_renderer_does_not_html_escape() {
    // In a `.css.rsc`, `<%= … %>` must not turn `>` into `&gt;`.
    let theme = Theme {
        accent: "hsl(0 100% > 50%)".into(),
    };
    assert_eq!(theme.render(), ".btn { color: hsl(0 100% > 50%); }");
}

#[test]
fn custom_renderer_drives_a_stock_component() {
    // Greeting was compiled with no knowledge of UpcaseRenderer.
    let g = Greeting {
        name: "ada".into(),
    };
    let mut r: Box<dyn Renderer> = Box::new(UpcaseRenderer::new());
    g.render_into(r.as_mut());
    assert_eq!(r.finish(), "HELLO ADA!");
}
