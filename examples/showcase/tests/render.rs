//! End-to-end behavior of the example components: escaping, composition,
//! control flow, and the custom-renderer seam.

use rsc::{Component, DEFAULT_SLOT, Render, Renderer, Slot, Slots, fragment};
use rsc_showcase::button::Button;
use rsc_showcase::card::Card;
use rsc_showcase::custom_renderer::UpcaseRenderer;
use rsc_showcase::greeting::Greeting;
use rsc_showcase::layout::Layout;
use rsc_showcase::list::List;
use rsc_showcase::menu::Menu;
use rsc_showcase::panel::Panel;

#[test]
fn escaped_interpolation() {
    let g = Greeting {
        name: "<Ada> & \"Co\"".into(),
    };
    assert_eq!(g.render(), "Hello &lt;Ada&gt; &amp; &quot;Co&quot;!");
}

#[test]
fn composition_same_buffer_and_string_path_agree() {
    // `Card` uses `{@render self.button}`; `Panel` uses `{@html self.button.render()}`.
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
fn snippet_render_prop() {
    // {#snippet item(label)} defines a parameterized fragment; {@render item(label)}
    // invokes it per element.
    let menu = Menu {
        labels: vec!["Home".into(), "A<B".into()],
    };
    assert_eq!(
        menu.render(),
        r#"<ul><li class="item">Home</li><li class="item">A&lt;B</li></ul>"#
    );
}

#[test]
fn component_element_with_scoped_use_and_slots() {
    use rsc_showcase::page::Page;
    let page = Page {
        heading: "Hi".into(),
        body: "World".into(),
        year: 2026,
    };
    assert_eq!(
        page.render(),
        r#"<div><section class="frame"><h2>Hi</h2><p>World</p><footer>© 2026</footer></section></div>"#
    );
}

#[test]
fn children_as_a_fragment_closure() {
    let body = fragment(|r: &mut dyn Renderer| {
        r.write_raw("<p>hi</p>");
    });
    let out = Layout.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &body)]));
    assert_eq!(out, "<main><p>hi</p></main>");
}

#[test]
fn children_as_a_component() {
    // A child component slotted into a slot host.
    let button = Button {
        label: "click".into(),
    };
    let out = Layout.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &button)]));
    assert_eq!(out, "<main><button>click</button></main>");
}

#[test]
fn unfilled_slot_renders_its_fallback() {
    use rsc_showcase::frame::Frame;
    // Neither slot is filled: the default slot is empty, the named one falls
    // back to the body of its `<slot>`.
    let frame = Frame {
        title: "Bare".into(),
    };
    assert_eq!(
        frame.render(),
        r#"<section class="frame"><h2>Bare</h2><footer>© anon</footer></section>"#
    );
}

#[test]
fn slots_are_matched_by_name_in_any_order() {
    use rsc_showcase::frame::Frame;
    let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>b</p>"));
    let foot = fragment(|r: &mut dyn Renderer| r.write_raw("f"));
    let frame = Frame { title: "T".into() };
    let out = frame.render_with(Slots::new(&[
        Slot::new("footer", &foot),
        Slot::new(DEFAULT_SLOT, &body),
    ]));
    assert_eq!(
        out,
        r#"<section class="frame"><h2>T</h2><p>b</p><footer>f</footer></section>"#
    );
}

#[test]
fn custom_renderer_drives_a_stock_component() {
    // Greeting was compiled with no knowledge of UpcaseRenderer.
    let g = Greeting { name: "ada".into() };
    let mut r: Box<dyn Renderer> = Box::new(UpcaseRenderer::new());
    g.render_into(r.as_mut());
    assert_eq!(r.finish(), "HELLO ADA!");
}

#[test]
fn slots_forward_through_a_wrapping_component() {
    // Shell fills Frame's slots with its own, so the caller's content lands two
    // levels down. A bare `<slot/>` forwards the default slot; the `<slot
    // name="footer">` fill wraps a placeholder that resolves against *Shell's*
    // caller, not Frame's.
    use rsc_showcase::shell::Shell;
    let body = fragment(|r: &mut dyn Renderer| r.write_raw("<p>b</p>"));
    let foot = fragment(|r: &mut dyn Renderer| r.write_raw("f"));
    let shell = Shell { title: "S".into() };
    let out = shell.render_with(Slots::new(&[
        Slot::new(DEFAULT_SLOT, &body),
        Slot::new("footer", &foot),
    ]));
    assert_eq!(
        out,
        r#"<section class="frame"><h2>S</h2><p>b</p><footer>f</footer></section>"#
    );
}

#[test]
fn forwarded_slot_falls_back_when_the_outer_caller_passes_nothing() {
    // Shell's own `<slot name="footer"/>` is unfilled, so Frame's footer fill
    // renders empty — Frame's fallback does not apply, because Frame's slot *was*
    // filled (with nothing).
    use rsc_showcase::shell::Shell;
    let shell = Shell { title: "S".into() };
    assert_eq!(
        shell.render(),
        r#"<section class="frame"><h2>S</h2><footer></footer></section>"#
    );
}

/// A `bool` attribute is present or absent, never `="false"` — the HTML rule,
/// since any value at all leaves the control disabled. An `Option` attribute
/// omits itself when `None`.
#[test]
fn conditional_attributes_are_present_or_absent() {
    use rsc_showcase::control::Control;

    let on = Control {
        disabled: true,
        placeholder: Some("name".into()),
        extra: None,
        invalid: false,
        compact: false,
        wiring: "",
        data: Vec::new(),
    }
    .render();
    assert!(on.contains(" disabled"), "{on}");
    assert!(!on.contains("disabled=\""), "{on}");
    assert!(on.contains(r#"placeholder="name""#), "{on}");

    let off = Control {
        disabled: false,
        placeholder: None,
        extra: None,
        invalid: false,
        compact: false,
        wiring: "",
        data: Vec::new(),
    }
    .render();
    assert!(!off.contains("disabled"), "{off}");
    assert!(!off.contains("placeholder"), "{off}");
}

/// The class list drops `None`, keeps first-mention order, dedupes, and lets a
/// `class:` directive overrule what the list produced.
#[test]
fn class_list_composes_and_directives_win() {
    use rsc_showcase::control::Control;

    let a = Control {
        disabled: false,
        placeholder: None,
        extra: Some("lead"),
        invalid: true,
        compact: true,
        wiring: "",
        data: Vec::new(),
    }
    .render();
    // `extra`, then the plain name, then the map entry whose condition holds.
    // `class:base={!self.invalid}` is false here, so it removes `base` even
    // though the list added it — the directive is the last word.
    assert!(a.contains(r#"class="lead invalid compact""#), "{a}");

    let b = Control {
        disabled: false,
        placeholder: None,
        extra: None,
        invalid: false,
        compact: false,
        wiring: "",
        data: Vec::new(),
    }
    .render();
    // `extra` is None and contributes nothing; the false map entry is absent;
    // `class:base` re-adds a name already present, which must not duplicate it.
    assert!(b.contains(r#"class="base""#), "{b}");
}

/// `{...expr}` splices attributes a component cannot name: a computed name, or
/// a map. The `&'static str` form is markup the author wrote; the pair form
/// escapes, so state can go through it safely.
#[test]
fn attribute_spread_splices_and_escapes() {
    use rsc_showcase::control::Control;

    let out = Control {
        disabled: false,
        placeholder: None,
        extra: None,
        invalid: false,
        compact: false,
        wiring: r#"data-relay-edit-target="name" hidden"#,
        data: vec![("data-host".to_string(), r#"" onclick="x"#.to_string())],
    }
    .render();

    assert!(
        out.contains(r#"data-relay-edit-target="name" hidden"#),
        "{out}"
    );
    // The pair form escapes, so a host named `" onclick=` stays a host name
    // rather than becoming an attribute of its own.
    assert!(!out.contains(r#"onclick="x""#), "{out}");
    assert!(out.contains("&quot;"), "{out}");
}

/// A prop whose type is `Option<_>` may be skipped at the call site; it arrives
/// as `None`, which the attribute rules then omit. A required prop still has to
/// be passed — leaving one out is a compile error, covered by `tests/ui`.
#[test]
fn optional_props_may_be_skipped() {
    use rsc_showcase::notice::Notice;

    let bare = Notice {
        title: "T".into(),
        detail: None,
        tone: None,
        dismissible: None,
    }
    .render();
    assert_eq!(bare, r#"<p class="notice">T</p>"#);

    // What the call site in `board.rsc` builds, naming none of the three.
    assert!(
        board().contains(r#"<p class="notice">Deploy finished</p>"#),
        "{}",
        board()
    );
}

/// An `Option` prop is first-class in the *value* too: a quoted attribute lands
/// inside the `Option` with no `Some(…)` at the call site — static text and
/// interpolated alike, and whatever string type the `Option` wraps.
#[test]
fn a_quoted_value_reaches_an_option_prop() {
    // detail="check {self.log}" → Option<String>, tone="warn" → Option<&str>,
    // and the bare `dismissible` → Option<bool>.
    assert!(
        board().contains(r#"title="check the log" data-tone="warn" data-dismissible>Rollback</p>"#),
        "{}",
        board()
    );
}

/// A generic component's builder carries its generics, so `note` is skippable
/// there too and `T` is still inferred from what the call site passes — the
/// setter takes the prop's type exactly, so `{42}` is an `i32` rather than an
/// ambiguity among every `From` impl.
#[test]
fn generic_component_skips_a_prop() {
    assert!(board().contains("<em>42</em>"), "{}", board());
}

/// `#[component(default)]` lets a call site skip any number of props; each one
/// it skips comes from the struct's `Default`, and the ones it sets win.
#[test]
fn defaulted_component_fills_in_the_rest() {
    assert!(
        board().contains(r#"<span class="indigo">Theme</span>"#),
        "{}",
        board()
    );
    assert!(
        board().contains(r#"<span class="indigo dense">Compact</span>"#),
        "{}",
        board()
    );
}

/// The page whose template skips props at every call site in it.
fn board() -> String {
    rsc_showcase::board::Board {
        log: "the log".into(),
    }
    .render()
}
