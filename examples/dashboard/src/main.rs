//! Renders the dashboard to stdout: `cargo run -p damask-dashboard > out.html`.

use damask::{Component, DEFAULT_SLOT, Slot, Slots};
use damask_dashboard::dashboard::Dashboard;
use damask_dashboard::demo_fleet;
use damask_dashboard::page::Page;

fn main() {
    let fleet = demo_fleet();
    let page = Page {
        title: "Fleet status · helm".into(),
        fleet: &fleet,
        nav: vec!["Overview", "Services", "Incidents"],
        current: "Overview",
        commit: "9f3c1ab7d20e".into(),
        year: 2026,
    };
    let dashboard = Dashboard {
        fleet: &fleet,
        feed_limit: 2,
    };
    println!(
        "{}",
        page.render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &dashboard)]))
    );
}
