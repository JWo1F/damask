//! The domain the page renders: a fleet of services and its deploy history.
//!
//! Templates stay declarative because every derived value — severity ordering,
//! rollups, formatting — is a method here. Anything more than a field access or
//! a comparison belongs in this file, not in a `{ … }` tag.

use std::fmt::{self, Display};

/// Operational state of a service.
///
/// Variants are ordered healthy → worst so `max()` over a fleet yields the
/// headline status, which is what [`Fleet::worst`] relies on.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Status {
    Healthy,
    Degraded,
    Down,
}

impl Status {
    /// Machine-readable form, used for CSS class suffixes and `data-` values.
    pub fn slug(self) -> &'static str {
        match self {
            Status::Healthy => "healthy",
            Status::Degraded => "degraded",
            Status::Down => "down",
        }
    }

    /// Human-readable form for badges and summaries.
    pub fn label(self) -> &'static str {
        match self {
            Status::Healthy => "Healthy",
            Status::Degraded => "Degraded",
            Status::Down => "Down",
        }
    }
}

/// `{ svc.status }` prints the label without an explicit call.
impl Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// One service in the fleet.
pub struct Service {
    pub name: String,
    pub owner: String,
    pub status: Status,
    /// Availability over the trailing window, as a percentage (`99.982`).
    pub uptime_pct: f64,
    /// p95 response time in milliseconds.
    pub latency_ms: u32,
    pub version: String,
}

/// Latency at or above this is called out in the table.
const SLOW_MS: u32 = 300;

impl Service {
    pub fn uptime(&self) -> String {
        format!("{:.3}%", self.uptime_pct)
    }

    pub fn latency(&self) -> String {
        format!("{} ms", self.latency_ms)
    }

    /// Whether p95 latency is bad enough to flag even when the service is up.
    pub fn is_slow(&self) -> bool {
        self.latency_ms >= SLOW_MS
    }

    /// Services that are down, or up but missing their availability target.
    pub fn breaches_slo(&self, target_pct: f64) -> bool {
        self.status == Status::Down || self.uptime_pct < target_pct
    }
}

/// A release of one service.
pub struct Deploy {
    pub service: String,
    pub version: String,
    pub author: String,
    pub minutes_ago: u32,
    /// Whether this deploy was rolled back after shipping.
    pub rolled_back: bool,
}

impl Deploy {
    /// Coarse relative time — enough for a feed, no date library needed.
    pub fn when(&self) -> String {
        match self.minutes_ago {
            0 => "just now".to_string(),
            m if m < 60 => format!("{m}m ago"),
            m if m < 60 * 24 => format!("{}h ago", m / 60),
            m => format!("{}d ago", m / (60 * 24)),
        }
    }
}

/// The whole fleet, plus the rollups the page headlines.
pub struct Fleet {
    pub services: Vec<Service>,
    pub deploys: Vec<Deploy>,
    /// The availability target every service is measured against.
    pub slo_target: f64,
}

impl Fleet {
    pub fn count(&self, status: Status) -> usize {
        self.services.iter().filter(|s| s.status == status).count()
    }

    /// The fleet headline: the most severe status any service is in. An empty
    /// fleet is reported healthy — there is nothing broken in it.
    pub fn worst(&self) -> Status {
        self.services
            .iter()
            .map(|s| s.status)
            .max()
            .unwrap_or(Status::Healthy)
    }

    /// Mean availability across the fleet, or 100% when there is nothing to
    /// average (avoids a NaN reaching the template).
    pub fn avg_uptime(&self) -> f64 {
        if self.services.is_empty() {
            return 100.0;
        }
        self.services.iter().map(|s| s.uptime_pct).sum::<f64>() / self.services.len() as f64
    }

    pub fn avg_uptime_label(&self) -> String {
        format!("{:.3}%", self.avg_uptime())
    }

    pub fn slo_label(&self) -> String {
        format!("{:.2}%", self.slo_target)
    }

    pub fn breaching(&self) -> usize {
        self.services
            .iter()
            .filter(|s| s.breaches_slo(self.slo_target))
            .count()
    }

    /// Whether anything needs attention — drives the banner in the header.
    pub fn all_clear(&self) -> bool {
        self.worst() == Status::Healthy && self.breaching() == 0
    }
}
