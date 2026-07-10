//! A complete HTML document assembled from composed RSC components.
//!
//! Where the `showcase` crate demonstrates one feature per component, this
//! example is a single realistic page: [`Page`](page::Page) is the document
//! shell (doctype, `<head>`, chrome) and takes any content as its default slot;
//! [`Dashboard`](dashboard::Dashboard) fills that slot and composes the rest.
//!
//! ```text
//! Page ── SiteHeader          nav + status banner
//!      ├─ <slot/> = Dashboard rollup tiles (snippet)
//!      │            ├─ ServiceTable ── StatusBadge
//!      │            └─ DeployFeed
//!      └─ SiteFooter          rollups + build info
//! ```
//!
//! Derived values live on the types in [`model`], so templates stay declarative.

pub mod dashboard;
pub mod deploy_feed;
pub mod model;
pub mod page;
pub mod service_table;
pub mod site_footer;
pub mod site_header;
pub mod status_badge;
pub mod theme;

use model::{Deploy, Fleet, Service, Status};

/// The fleet the binary and the tests both render.
pub fn demo_fleet() -> Fleet {
    Fleet {
        slo_target: 99.9,
        services: vec![
            Service {
                name: "edge-router".into(),
                owner: "platform".into(),
                status: Status::Healthy,
                uptime_pct: 99.995,
                latency_ms: 42,
                version: "v2.14.0".into(),
            },
            Service {
                name: "checkout-api".into(),
                owner: "payments".into(),
                status: Status::Degraded,
                uptime_pct: 99.812,
                latency_ms: 380,
                version: "v5.1.2".into(),
            },
            Service {
                name: "search-index".into(),
                owner: "discovery".into(),
                status: Status::Healthy,
                uptime_pct: 99.940,
                latency_ms: 310,
                version: "v0.9.7".into(),
            },
            Service {
                name: "image-resizer".into(),
                owner: "media".into(),
                status: Status::Down,
                uptime_pct: 97.400,
                latency_ms: 0,
                // Escaping check: this renders as text, not as markup.
                version: "v1.0.0-rc<1>".into(),
            },
        ],
        deploys: vec![
            Deploy {
                service: "checkout-api".into(),
                version: "v5.1.2".into(),
                author: "ada".into(),
                minutes_ago: 12,
                rolled_back: false,
            },
            Deploy {
                service: "image-resizer".into(),
                version: "v1.0.0-rc<1>".into(),
                author: "grace".into(),
                minutes_ago: 95,
                rolled_back: true,
            },
            Deploy {
                service: "edge-router".into(),
                version: "v2.14.0".into(),
                author: "linus".into(),
                minutes_ago: 1500,
                rolled_back: false,
            },
        ],
    }
}
