//! End-to-end rendering tests for the composed page.

use damask::{Component, DEFAULT_SLOT, Slot, Slots};
use damask_dashboard::dashboard::Dashboard;
use damask_dashboard::deploy_feed::DeployFeed;
use damask_dashboard::model::{Deploy, Fleet, Service, Status};
use damask_dashboard::page::Page;
use damask_dashboard::site_header::SiteHeader;
use damask_dashboard::status_badge::StatusBadge;
use damask_dashboard::{demo_fleet, service_table::ServiceTable};

fn page(fleet: &Fleet) -> String {
    let dashboard = Dashboard {
        fleet,
        feed_limit: 2,
    };
    Page {
        title: "Fleet status".into(),
        fleet,
        nav: vec!["Overview", "Services"],
        current: "Overview",
        commit: "9f3c1ab7d20e".into(),
        year: 2026,
    }
    .render_with(Slots::new(&[Slot::new(DEFAULT_SLOT, &dashboard)]))
}

fn healthy_fleet() -> Fleet {
    Fleet {
        slo_target: 99.9,
        services: vec![Service {
            name: "edge".into(),
            owner: "platform".into(),
            status: Status::Healthy,
            uptime_pct: 99.99,
            latency_ms: 20,
            version: "v1".into(),
        }],
        deploys: Vec::new(),
    }
}

#[test]
fn renders_a_complete_document() {
    let out = page(&demo_fleet());
    assert!(
        out.starts_with("<!DOCTYPE html>"),
        "doctype leads: {out:.40}"
    );
    for fragment in [
        "<html lang=\"en\">",
        "<meta charset=\"utf-8\">",
        "<title>Fleet status</title>",
        "<body>",
        "</html>",
    ] {
        assert!(out.contains(fragment), "missing {fragment:?}");
    }
}

/// The stylesheet is written with `{@html … }`, so it must reach the page
/// verbatim. Asserted against the `<style>` block's contents rather than a
/// formatted substring, so reformatting `theme.css` doesn't break the test.
#[test]
fn stylesheet_is_inlined_raw() {
    let out = page(&demo_fleet());
    let style = out
        .split_once("<style>")
        .and_then(|(_, rest)| rest.split_once("</style>"))
        .map(|(css, _)| css)
        .expect("page has a <style> block");

    assert!(style.contains(".badge.down"), "stylesheet missing");
    assert!(style.contains("box-sizing"), "declarations missing");
    // Braces reach the page as braces — the parser never saw them as tags.
    assert!(style.contains('{') && style.contains('}'), "braces lost");
    // A quoted font name is the tell: `{ … }` would have escaped `"` to
    // `&quot;`, so finding the quote intact proves the CSS went out unescaped.
    assert!(style.contains("\"Menlo\""), "quotes escaped: css not raw");
    assert!(!style.contains("&quot;"), "css was escaped");
}

#[test]
fn nested_components_all_render() {
    let out = page(&demo_fleet());
    // Page → SiteHeader / Dashboard → ServiceTable → StatusBadge, and DeployFeed.
    assert!(out.contains("class=\"masthead\""), "SiteHeader missing");
    assert!(out.contains("class=\"tiles\""), "Dashboard tiles missing");
    assert!(out.contains("<table>"), "ServiceTable missing");
    assert!(out.contains("data-status=\"down\""), "StatusBadge missing");
    assert!(out.contains("class=\"feed\""), "DeployFeed missing");
    assert!(out.contains("<footer>"), "SiteFooter missing");
}

#[test]
fn interpolation_escapes_but_raw_html_does_not() {
    let out = page(&demo_fleet());
    assert!(out.contains("v1.0.0-rc&lt;1&gt;"), "version not escaped");
    assert!(!out.contains("v1.0.0-rc<1>"), "unescaped version leaked");
}

#[test]
fn rollups_match_the_fleet() {
    let fleet = demo_fleet();
    assert_eq!(fleet.count(Status::Healthy), 2);
    assert_eq!(fleet.count(Status::Degraded), 1);
    assert_eq!(fleet.count(Status::Down), 1);
    // Down, plus checkout-api at 99.812 under the 99.9 target.
    assert_eq!(fleet.breaching(), 2);
    assert_eq!(fleet.worst(), Status::Down);
    assert!(!fleet.all_clear());

    let out = page(&fleet);
    assert!(out.contains("<div class=\"n\">2</div><div class=\"k\">healthy</div>"));
    assert!(out.contains("<div class=\"n\">1</div><div class=\"k\">down</div>"));
    assert!(out.contains("<div class=\"n\">2</div><div class=\"k\">below SLO</div>"));
}

#[test]
fn banner_follows_worst_status() {
    let alert = page(&demo_fleet());
    assert!(alert.contains("class=\"banner alert\""));
    assert!(alert.contains("1 service(s) down"));

    let ok = page(&healthy_fleet());
    assert!(ok.contains("class=\"banner ok\""), "healthy fleet banner");
    assert!(ok.contains("All 1 services are healthy"));
}

#[test]
fn nav_marks_only_the_current_entry() {
    let fleet = demo_fleet();
    let out = SiteHeader {
        fleet: &fleet,
        nav: vec!["Overview", "Services"],
        current: "Services",
    }
    .render();
    assert!(out.contains(r#"<a href="/services" class="active">Services</a>"#));
    assert!(out.contains(r#"<a href="/" class="">Overview</a>"#));
}

#[test]
fn rows_get_stripe_and_breach_classes() {
    let fleet = demo_fleet();
    let out = ServiceTable {
        services: &fleet.services,
        slo_target: fleet.slo_target,
    }
    .render();
    // Index 0, healthy and above target — neither class.
    assert!(out.contains(r#"<tr class="">"#));
    // Index 1 and 3 are odd *and* breaching.
    assert_eq!(out.matches(r#"<tr class="alt breach">"#).count(), 2);
    // Only latencies at/above the 300ms threshold are flagged.
    assert_eq!(out.matches(r#"<span class="flag">slow</span>"#).count(), 2);
}

#[test]
fn badge_class_and_label_come_from_the_status() {
    for (status, slug, label) in [
        (Status::Healthy, "healthy", "Healthy"),
        (Status::Degraded, "degraded", "Degraded"),
        (Status::Down, "down", "Down"),
    ] {
        let out = StatusBadge { status }.render();
        assert!(out.contains(&format!(r#"class="badge {slug}""#)), "{out}");
        assert!(out.contains(&format!(">{label}</span>")), "{out}");
    }
}

#[test]
fn feed_shows_limit_then_summarises_the_rest() {
    let fleet = demo_fleet();
    let out = DeployFeed {
        deploys: &fleet.deploys,
        limit: 2,
    }
    .render();
    assert_eq!(out.matches("<li>").count(), 2, "limit not applied");
    assert!(out.contains("and 1 older deploy(s) not shown."));
    assert!(out.contains(r#"<span class="rb">rolled back</span>"#));
    // Relative times bucket by magnitude.
    assert!(out.contains("12m ago") && out.contains("1h ago"));
}

#[test]
fn empty_feed_renders_the_empty_state() {
    let out = DeployFeed {
        deploys: &[],
        limit: 5,
    }
    .render();
    assert!(
        out.contains(r#"<p class="empty">"#),
        "no empty state: {out}"
    );
    assert!(!out.contains("<li>"), "empty feed listed items");
}

#[test]
fn relative_time_buckets() {
    let at = |minutes_ago| {
        Deploy {
            service: "s".into(),
            version: "v".into(),
            author: "a".into(),
            minutes_ago,
            rolled_back: false,
        }
        .when()
    };
    assert_eq!(at(0), "just now");
    assert_eq!(at(59), "59m ago");
    assert_eq!(at(60), "1h ago");
    assert_eq!(at(60 * 24), "1d ago");
}

#[test]
fn empty_fleet_has_no_nan_rollups() {
    let empty = Fleet {
        slo_target: 99.9,
        services: Vec::new(),
        deploys: Vec::new(),
    };
    assert_eq!(empty.worst(), Status::Healthy);
    assert!(empty.all_clear());
    let out = page(&empty);
    assert!(!out.contains("NaN"), "NaN reached the page");
    assert!(out.contains("All 0 services are healthy"));
}
