//! What reaches the page, end to end: a real component, built through the
//! builder a `<Passthrough …/>` tag lowers to, rendered as a string.

use damask::{Attrs, Component};
use damask_attributes::page::Page;
use damask_attributes::passthrough::{Passthrough, Seeded};
use damask_attributes::strict::Strict;

fn attrs(pairs: &[(&'static str, &'static str)]) -> Attrs {
    let mut attrs = Attrs::new();
    for (name, value) in pairs {
        attrs.insert_static(name, value);
    }
    attrs
}

#[test]
fn an_attribute_the_component_does_not_name_reaches_the_page() {
    let control = Passthrough {
        attrs: attrs(&[("data-cover-target", "input"), ("aria-label", "Email")]),
        ..Passthrough::default()
    };
    assert_eq!(
        control.render().trim_end(),
        r#"<input type="text" data-cover-target="input" aria-label="Email">"#
    );
}

#[test]
fn a_bare_attribute_is_written_bare_and_a_false_one_not_at_all() {
    let mut bag = Attrs::new();
    bag.insert("autofocus", true);
    bag.insert("disabled", false);
    bag.insert("readonly", Option::<String>::None);
    let control = Passthrough {
        attrs: bag,
        ..Passthrough::default()
    };
    assert_eq!(
        control.render().trim_end(),
        r#"<input type="text" autofocus>"#
    );
}

/// Escaping is what a bag buys over markup assembled by hand: each value is
/// escaped on its own, so a quote in one cannot end the attribute.
#[test]
fn values_are_escaped_one_at_a_time() {
    let mut bag = Attrs::new();
    bag.insert("data-note", r#"a "quoted" & <angled> value"#);
    let control = Passthrough {
        attrs: bag,
        ..Passthrough::default()
    };
    assert_eq!(
        control.render().trim_end(),
        r#"<input type="text" data-note="a &quot;quoted&quot; &amp; &lt;angled&gt; value">"#
    );
}

/// A name given twice keeps its first position and takes its last value, so a
/// default a component filled in can be overridden without the output
/// reshuffling.
#[test]
fn a_name_given_twice_keeps_its_place_and_takes_the_last_value() {
    let mut bag = Attrs::new();
    bag.insert("data-a", "1");
    bag.insert("data-b", "2");
    bag.insert("data-a", "3");
    let control = Passthrough {
        attrs: bag,
        ..Passthrough::default()
    };
    assert_eq!(
        control.render().trim_end(),
        r#"<input type="text" data-a="3" data-b="2">"#
    );
}

/// The element writes its own `type`, so a `type` in the bag is dropped rather
/// than written into the same tag twice.
#[test]
fn the_elements_own_attributes_win_over_the_spread() {
    let control = Passthrough {
        kind: Some("email".into()),
        attrs: attrs(&[("type", "tel"), ("data-ok", "1")]),
        ..Passthrough::default()
    };
    assert_eq!(
        control.render().trim_end(),
        r#"<input type="email" data-ok="1">"#
    );
}

/// Every call site in `page.dmk`, which is where the feature is actually
/// written down.
#[test]
fn the_call_sites_render_as_written() {
    let page = Page {
        email: Some("ada@example.com".into()),
        tracking: vec![("data-campaign".into(), "spring".into())],
    };
    let html = page.render();

    // A prop is read by the component; the rest ride along.
    assert!(
    html.contains(
      r#"<input type="email" value="ada@example.com" data-signup-target="email" aria-label="Email address" class="field" autofocus>"#
    ),
    "{html}"
  );
    // `type` at a call site is a keyword, so it goes to the bag — and is then
    // dropped, because the element writes its own.
    assert!(html.contains(r#"<input type="email">"#), "{html}");
    assert_eq!(html.matches(r#"type="tel""#).count(), 0, "{html}");
    // A wrapper hands its own bag on to the component inside it.
    assert!(
        html.contains(r#"data-signup-target="confirm" placeholder="Confirm the address""#),
        "{html}"
    );
    // A set assembled in Rust, spread the way one is onto an element.
    assert!(html.contains(r#"data-campaign="spring""#), "{html}");
    // And `{row.class()}` still reaches `Row::class`, not the fallback trait
    // `class="field"` above put in this template's scope.
    assert_eq!(
        html.matches(r#"<label class="row required">"#).count(),
        2,
        "{html}"
    );
}

#[test]
fn a_component_without_a_bag_renders_its_own_props() {
    let strict = Strict {
        title: Some("Sign up".into()),
    };
    assert_eq!(strict.render().trim_end(), "<h1>Sign up</h1>");
}

/// A bag is a collection, so a call site adds to what `Default` seeded rather
/// than replacing it. The alternative loses a component's own defaults the
/// moment a call site writes any attribute at all, with nothing saying so.
#[test]
fn a_call_site_adds_to_a_bag_its_default_seeded() {
    let mut written = Attrs::new();
    written.insert_static("class", "btn danger");
    written.insert_static("data-turbo-frame", "_top");

    let seeded = Seeded::__damask_props().attrs(written).__damask_build();

    assert_eq!(
        seeded.render().trim_end(),
        r#"<button class="btn danger" data-role="button" data-turbo-frame="_top"></button>"#
    );
}

/// And with nothing written, the default is what it always was.
#[test]
fn a_seeded_bag_is_untouched_when_a_call_site_writes_nothing() {
    assert_eq!(
        Seeded::default().render().trim_end(),
        r#"<button class="btn" data-role="button"></button>"#
    );
}
