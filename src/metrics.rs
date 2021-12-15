use once_cell::sync::Lazy;
use prometheus::{register_counter_vec, register_histogram_vec, CounterVec, HistogramVec};

pub(crate) static TICK_DURATION: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "instance_fetch_duration_ms",
        "Time it took gearbox-maintenance to fetch data from one transmission instance",
        &["transmission_url"]
    )
    .unwrap()
});

pub(crate) static TICK_FAILURES: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "instance_fetch_failure_count",
        "Number of times that fetching from the instance failed",
        &["transmission_url"]
    )
    .unwrap()
});

pub(crate) static TORRENT_SIZES: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "torrent_sizes_bytes",
        "Histogram of torrent size. Use sum and count to see total size on a tracker, and count of torrents.",
        &["transmission_url", "policy"],
        prometheus::exponential_buckets(0.5e9, 2.0, 11).unwrap() // 500MB, then up to 1 terabyte
    )
    .unwrap()
});

pub(crate) static TORRENT_DELETIONS: Lazy<CounterVec> = Lazy::new(|| {
    register_counter_vec!(
        "torrent_deletion_count",
        "Number of torrents that got deleted, per instance/policy",
        &["transmission_url", "policy"]
    )
    .unwrap()
});

/// Adds a 1 to a `prometheus::core::GenericCounter` when it is dropped.
pub(crate) struct FailureCounter<P: prometheus::core::Atomic>(
    prometheus::core::GenericCounter<P>,
    bool,
);

impl<P: prometheus::core::Atomic> FailureCounter<P> {
    /// Create a failure counter that increments a prometheus counter unless told not to.
    pub(crate) fn new(counter: prometheus::core::GenericCounter<P>) -> Self {
        Self(counter, true)
    }

    /// Declare the operation a success, and don't increment the failure counter.
    pub(crate) fn succeed(self) {
        let mut fc = self;
        fc.1 = false;
    }
}

impl<P: prometheus::core::Atomic> Drop for FailureCounter<P> {
    fn drop(&mut self) {
        if self.1 {
            self.0.inc();
        }
    }
}
