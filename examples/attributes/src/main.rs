use damask::Component;
use damask_attributes::page::Page;

fn main() {
    let page = Page {
        email: Some("ada@example.com".into()),
        tracking: vec![
            ("data-campaign".into(), "spring".into()),
            // Escaped on the way out, which hand-assembled markup could not promise.
            ("data-note".into(), r#"a "quoted" & <angled> value"#.into()),
        ],
    };
    println!("{}", page.render());
}
