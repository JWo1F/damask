//! `tag!` from outside the crate, which is the path every user takes: the
//! `$crate` in the wrapper has to resolve to `::damask` here rather than to
//! `crate`, and the attribute machinery has to be reachable through it.

use damask::{Render, ToTrusted, Trusted, tag};

#[test]
fn an_element_with_nothing_in_it() {
    assert_eq!(tag!(div).as_str(), "<div></div>");
}

#[test]
fn a_void_element_has_no_closing_tag() {
    assert_eq!(tag!(br).as_str(), "<br>");
    assert_eq!(
        tag!(input, r#type: "text", name: "q").as_str(),
        r#"<input type="text" name="q">"#
    );
}

#[test]
fn text_content_is_escaped() {
    assert_eq!(
        tag!(p, "1 < 2 & \"quoted\"").as_str(),
        "<p>1 &lt; 2 &amp; &quot;quoted&quot;</p>"
    );
}

#[test]
fn markup_content_is_spliced() {
    assert_eq!(tag!(p, tag!(b, "bold")).as_str(), "<p><b>bold</b></p>");
}

#[test]
fn a_tuple_mixes_markup_and_text_without_boxing() {
    let markup = tag!(p, (tag!(b, "a"), " & ", 2_u8));
    assert_eq!(markup.as_str(), "<p><b>a</b> &amp; 2</p>");
}

#[test]
fn a_list_of_elements_is_content() {
    let items: Vec<Trusted> = ["a", "b"].iter().map(|t| tag!(li, *t)).collect();
    assert_eq!(tag!(ul, items).as_str(), "<ul><li>a</li><li>b</li></ul>");
}

#[test]
fn an_absent_child_writes_nothing() {
    assert_eq!(tag!(p, None::<&str>).as_str(), "<p></p>");
    assert_eq!(tag!(p, Some("x")).as_str(), "<p>x</p>");
}

#[test]
fn attribute_values_are_escaped() {
    assert_eq!(
        tag!(a, href: "/?a=1&b=2", "go").as_str(),
        r#"<a href="/?a=1&amp;b=2">go</a>"#
    );
}

#[test]
fn a_boolean_attribute_is_bare_or_absent() {
    assert_eq!(tag!(input, disabled: true).as_str(), "<input disabled>");
    assert_eq!(tag!(input, disabled: false).as_str(), "<input>");
}

#[test]
fn an_absent_attribute_declines_to_appear() {
    assert_eq!(tag!(input, value: None::<&str>).as_str(), "<input>");
    assert_eq!(
        tag!(input, value: Some("x")).as_str(),
        r#"<input value="x">"#
    );
}

#[test]
fn an_ident_attribute_name_spells_a_hyphen_with_an_underscore() {
    assert_eq!(
        tag!(div, aria_label: "Close").as_str(),
        r#"<div aria-label="Close"></div>"#
    );
    assert_eq!(
        tag!(meta, "http-equiv": "refresh", content: "0").as_str(),
        r#"<meta http-equiv="refresh" content="0">"#
    );
}

#[test]
fn an_id_can_be_written_in_the_head_or_as_an_attribute() {
    assert_eq!(tag!(div #main).as_str(), r#"<div id="main"></div>"#);
    assert_eq!(tag!(div, id: "main").as_str(), r#"<div id="main"></div>"#);
}

#[test]
fn class_takes_the_three_forms_a_template_takes() {
    let active = true;
    assert_eq!(
        tag!(div, class: "card wide").as_str(),
        r#"<div class="card wide"></div>"#
    );
    assert_eq!(
        tag!(div, class: ["card", active.then_some("is-active"), None::<&str>]).as_str(),
        r#"<div class="card is-active"></div>"#
    );
    assert_eq!(
        tag!(div, class: { "is-active": active, "is-muted": !active }).as_str(),
        r#"<div class="is-active"></div>"#
    );
}

#[test]
fn a_class_named_twice_appears_once() {
    assert_eq!(
        tag!(div, class: ["card", "card wide"]).as_str(),
        r#"<div class="card wide"></div>"#
    );
}

#[test]
fn a_data_boolean_is_bare_or_absent_as_it_is_in_a_template() {
    assert_eq!(
        tag!(div, data: { par1: true, par2: false }).as_str(),
        "<div data-par1></div>"
    );
}

#[test]
fn a_data_key_is_taken_as_written() {
    assert_eq!(
        tag!(div, data: { user_id: 7, "row-index": 2 }).as_str(),
        r#"<div data-user_id="7" data-row-index="2"></div>"#
    );
}

#[test]
fn data_takes_a_whole_set_too() {
    let extra = [("controller", "modal"), ("action", "click->modal#close")];
    assert_eq!(
        tag!(div, data: extra).as_str(),
        r#"<div data-controller="modal" data-action="click-&gt;modal#close"></div>"#
    );
}

#[test]
fn attributes_are_written_in_the_order_they_are_given() {
    assert_eq!(
        tag!(div, class: "a", data: { x: 1 }, title: "t").as_str(),
        r#"<div class="a" data-x="1" title="t"></div>"#
    );
}

#[test]
fn nesting_holds_a_whole_document_together() {
    let rows = [("Coffee", 3.5_f64), ("Rent", 900.0)];
    let body: Vec<Trusted> = rows
        .iter()
        .map(|(name, amount)| tag!(tr, (tag!(td, *name), tag!(td, *amount))))
        .collect();
    assert_eq!(
        tag!(table, class: "ledger", body).as_str(),
        r#"<table class="ledger"><tr><td>Coffee</td><td>3.5</td></tr><tr><td>Rent</td><td>900</td></tr></table>"#
    );
}

#[test]
fn a_style_element_carries_css_a_template_could_not_hold() {
    // The `{` a `.dmk` reserves is an ordinary character in a Rust string, so a
    // stylesheet needs no file of its own to escape the template language.
    let css = ".card { padding: 1rem; }";
    assert_eq!(
        tag!(style, css.to_trusted()).as_str(),
        "<style>.card { padding: 1rem; }</style>"
    );
}

#[test]
fn an_element_is_renderable_so_it_composes_with_components() {
    let markup = tag!(em, "x");
    let mut r = damask::HtmlRenderer::new();
    markup.render_into(&mut r);
    assert_eq!(r.as_str(), "<em>x</em>");
}

#[test]
fn what_a_user_typed_stays_data_however_deep_it_goes() {
    let typed = "<script>alert(1)</script>";
    assert_eq!(
        tag!(div, tag!(p, typed)).as_str(),
        "<div><p>&lt;script&gt;alert(1)&lt;/script&gt;</p></div>"
    );
}
