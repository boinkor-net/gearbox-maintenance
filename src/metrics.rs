use std::{sync::Arc, time::SystemTime};

use axum::{
    body::Body,
    extract::State,
    http::{header::CONTENT_TYPE, Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use prometheus_client::{
    encoding::{text::encode, EncodeLabelSet},
    metrics::{
        counter::Counter,
        family::Family,
        gauge::Gauge,
        histogram::{exponential_buckets, Histogram},
    },
    registry::Registry,
};
use tokio::sync::Mutex;

pub(crate) struct TickDurationHandle {
    family: Family<TransmissionLocation, Histogram>,
    variant: TransmissionLocation,
    started: SystemTime,
}

impl Drop for TickDurationHandle {
    fn drop(&mut self) {
        if let Ok(duration) = self.started.elapsed() {
            self.family
                .get_or_create(&self.variant)
                .observe(duration.as_millis() as f64);
        }
    }
}

pub(crate) struct FailureCountHandle {
    family: Family<TransmissionLocation, Counter>,
    variant: TransmissionLocation,
    success: bool,
}

impl FailureCountHandle {
    pub fn succeed(mut self) {
        self.success = true;
    }
}

impl Drop for FailureCountHandle {
    fn drop(&mut self) {
        if !self.success {
            self.family.get_or_create(&self.variant).inc();
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct TransmissionLocation {
    transmission_url: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub(crate) struct Policy {
    transmission_url: String,
    policy: String,
}

impl Policy {
    pub(crate) fn new_for(transmission_url: &str, policy: &str) -> Self {
        Policy {
            transmission_url: transmission_url.to_string(),
            policy: policy.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Metrics {
    tick_duration: Family<TransmissionLocation, Histogram>,
    tick_failure_counter: Family<TransmissionLocation, Counter>,
    size_distribution: Family<Policy, Histogram>,
    torrent_deletions: Family<Policy, Counter>,
    total_count: Family<Policy, Gauge>,
    total_size: Family<Policy, Gauge>,
}

impl Metrics {
    /// Initialize and return a set of metrics registered on `registry`.
    pub(crate) fn for_registry(registry: &mut Registry) -> Self {
        let metrics = Self {
            tick_duration: Family::new_with_constructor(|| {
                Histogram::new(exponential_buckets(1.0, 1.5, 20))
            }),
            tick_failure_counter: Family::default(),
            size_distribution: Family::new_with_constructor(|| {
                Histogram::new(exponential_buckets(5e9, 2.0, 11))
            }),
            torrent_deletions: Family::default(),
            total_count: Family::default(),
            total_size: Family::default(),
        };
        registry.register(
            "instance_fetch_duration_ms",
            "Time it took gearbox-maintenance to fetch data from one transmission instance",
            metrics.tick_duration.clone(),
        );
        registry.register(
            "instance_fetch_failure_count",
            "Number of times that fetching from the instance failed",
            metrics.tick_failure_counter.clone(),
        );
        registry.register(
            "torrent_size_bytes_historam",
            "Histogram of torrent size managed by policy.",
            metrics.size_distribution.clone(),
        );
        registry.register(
            "torrent_deletion_count",
            "Number of torrents that got deleted, per instance/policy",
            metrics.torrent_deletions.clone(),
        );
        registry.register(
            "torrent_count",
            "Number of torrents, per transmission URL and policy.",
            metrics.total_count.clone(),
        );
        registry.register(
            "torrent_size_bytes",
            "Total data size of torrents in bytes, per transmission URL and policy.",
            metrics.total_size.clone(),
        );

        metrics
    }

    /// Return a histogram timer that tracks the duration it is in scope.
    pub(crate) fn tick_duration(&self, url: &str) -> TickDurationHandle {
        TickDurationHandle {
            family: self.tick_duration.clone(),
            variant: TransmissionLocation {
                transmission_url: url.to_string(),
            },
            started: SystemTime::now(),
        }
    }

    /// Return a [`FailureTracker`] that will record a failure when it goes out of scope.
    pub(crate) fn tick_failure_tracker(&self, url: &str) -> FailureCountHandle {
        FailureCountHandle {
            family: self.tick_failure_counter.clone(),
            variant: TransmissionLocation {
                transmission_url: url.to_string(),
            },
            success: false,
        }
    }

    /// Track a torrent's size on the size distribution histogram.
    pub(crate) fn track_size(&self, policy: &Policy, size: usize) {
        self.size_distribution
            .get_or_create(policy)
            .observe(size as f64);
    }

    /// Track a torrent deletion.
    pub(crate) fn track_torrent_deletion(&self, policy: &Policy) {
        self.torrent_deletions.get_or_create(policy).inc();
    }

    pub(crate) fn update_count(&self, policy: &Policy, count: usize) {
        self.total_count.get_or_create(policy).set(count as i64);
    }

    pub(crate) fn update_size(&self, policy: &Policy, size: usize) {
        self.total_size.get_or_create(policy).set(size as i64);
    }
}

struct AppState {
    pub registry: Registry,
}

async fn metrics_handler(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let state = state.lock().await;
    let mut buffer = String::new();
    encode(&mut buffer, &state.registry).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )
        .body(Body::from(buffer))
        .unwrap()
}

pub(crate) fn metrics_router(registry: Registry) -> Router {
    let state = Arc::new(Mutex::new(AppState { registry }));

    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}
