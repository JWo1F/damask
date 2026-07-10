//! Renders the dashboard to stdout: `cargo run -p rsc-dashboard > out.html`.

use rsc::Component;
use rsc_dashboard::dashboard::Dashboard;
use rsc_dashboard::demo_fleet;
use rsc_dashboard::page::Page;

fn main() {
    let fleet = demo_fleet();
    let page = Page {
        title: "Fleet status · helm".into(),
        fleet: &fleet,
        nav: vec!["Overview", "Services", "Incidents"],
        current: "Overview",
        commit: "9f3c1ab7d20e".into(),
        year: 2026,
        children: Dashboard {
            fleet: &fleet,
            feed_limit: 2,
        },
    };
    println!("{}", page.render());
}
