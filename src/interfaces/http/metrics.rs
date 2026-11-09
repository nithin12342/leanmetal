//! src/interfaces/http/metrics.rs
//! File ID: FILE-014
//! Responsibility: Render OpenMetrics Prometheus exposition output (<=7 words)
//! Must Never: Block evaluation threads or allocate unbounded exposition buffers.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

/// Lock-free daemon telemetry (REQ-019 / SPEC-019).
/// Records evaluation counts, latency sums, L1 hit ratios,
/// WAL depth (via AsyncWal) and pruned-client counts.
#[derive(Clone, Default)]
pub struct DaemonMetrics {
    inner: Arc<MetricsInner>,
}

#[derive(Default)]
struct MetricsInner {
    evaluations_total: AtomicU64,
    eval_latency_sum_us: AtomicU64,
    eval_latency_max_us: AtomicU64,
    l1_hits: AtomicU64,
    l1_misses: AtomicU64,
    slow_pruned_total: AtomicU64,
    xdp_rx_packets: AtomicU64,
    xdp_rx_bytes: AtomicU64,
}

impl DaemonMetrics {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner::default()),
        }
    }

    /// Records one completed evaluation (fast-path or dynamic).
    #[inline(always)]
    pub fn record_evaluation(&self, latency_us: u64, served_from_l1: bool) {
        self.inner.evaluations_total.fetch_add(1, Ordering::Relaxed);
        self.inner
            .eval_latency_sum_us
            .fetch_add(latency_us, Ordering::Relaxed);
        self.inner
            .eval_latency_max_us
            .fetch_max(latency_us, Ordering::Relaxed);
        if served_from_l1 {
            self.inner.l1_hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.inner.l1_misses.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Records slow-consumer prune events (backpressure hub).
    pub fn record_pruned(&self, n: u64) {
        self.inner
            .slow_pruned_total
            .fetch_add(n, Ordering::Relaxed);
    }

    /// Records native AF_XDP packets ingested off-NIC (REQ-020 / SPEC-020).
    pub fn record_xdp_rx(&self, packets: u64, bytes: u64) {
        self.inner.xdp_rx_packets.fetch_add(packets, Ordering::Relaxed);
        self.inner.xdp_rx_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn evaluations_total(&self) -> u64 {
        self.inner.evaluations_total.load(Ordering::Relaxed)
    }

    /// Renders OpenMetrics / Prometheus text exposition (version 0.0.4).
    pub fn render_prometheus(
        &self,
        wal_queue_depth: u64,
        active_websockets: usize,
        valkey_connected: bool,
    ) -> String {
        let total = self.inner.evaluations_total.load(Ordering::Relaxed);
        let sum = self.inner.eval_latency_sum_us.load(Ordering::Relaxed);
        let max = self.inner.eval_latency_max_us.load(Ordering::Relaxed);
        let hits = self.inner.l1_hits.load(Ordering::Relaxed);
        let misses = self.inner.l1_misses.load(Ordering::Relaxed);
        let pruned = self.inner.slow_pruned_total.load(Ordering::Relaxed);
        let xdp_packets = self.inner.xdp_rx_packets.load(Ordering::Relaxed);
        let xdp_bytes = self.inner.xdp_rx_bytes.load(Ordering::Relaxed);
        let avg = sum.checked_div(total).unwrap_or(0);
        let hit_ratio = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        format!(
            "# HELP edgeflag_evaluations_total Total flag evaluations served.\n\
             # TYPE edgeflag_evaluations_total counter\n\
             edgeflag_evaluations_total {total}\n\
             # HELP edgeflag_eval_latency_avg_us Mean evaluation latency in microseconds.\n\
             # TYPE edgeflag_eval_latency_avg_us gauge\n\
             edgeflag_eval_latency_avg_us {avg}\n\
             # HELP edgeflag_eval_latency_max_us Max observed evaluation latency in microseconds.\n\
             # TYPE edgeflag_eval_latency_max_us gauge\n\
             edgeflag_eval_latency_max_us {max}\n\
             # HELP edgeflag_l1_cache_hits_total L1 TinyUFO cache hits.\n\
             # TYPE edgeflag_l1_cache_hits_total counter\n\
             edgeflag_l1_cache_hits_total {hits}\n\
             # HELP edgeflag_l1_cache_misses_total L1 cache misses (dynamic path).\n\
             # TYPE edgeflag_l1_cache_misses_total counter\n\
             edgeflag_l1_cache_misses_total {misses}\n\
             # HELP edgeflag_l1_hit_ratio L1 hit ratio 0.0-1.0.\n\
             # TYPE edgeflag_l1_hit_ratio gauge\n\
             edgeflag_l1_hit_ratio {hit_ratio:.4}\n\
             # HELP edgeflag_wal_queue_depth Async WAL pending writes.\n\
             # TYPE edgeflag_wal_queue_depth gauge\n\
             edgeflag_wal_queue_depth {wal_queue_depth}\n\
             # HELP edgeflag_active_websockets Currently connected WebSocket clients.\n\
             # TYPE edgeflag_active_websockets gauge\n\
             edgeflag_active_websockets {active_websockets}\n\
             # HELP edgeflag_valkey_connected 1 if linked to Valkey mesh, else 0.\n\
             # TYPE edgeflag_valkey_connected gauge\n\
             edgeflag_valkey_connected {}\n\
             # HELP edgeflag_slow_consumers_pruned_total Slow consumers pruned by backpressure.\n\
             # TYPE edgeflag_slow_consumers_pruned_total counter\n\
             edgeflag_slow_consumers_pruned_total {pruned}\n\
             # HELP edgeflag_xdp_rx_packets_total Packets ingested via native AF_XDP.\n\
             # TYPE edgeflag_xdp_rx_packets_total counter\n\
             edgeflag_xdp_rx_packets_total {xdp_packets}\n\
             # HELP edgeflag_xdp_rx_bytes_total Bytes ingested via native AF_XDP.\n\
             # TYPE edgeflag_xdp_rx_bytes_total counter\n\
             edgeflag_xdp_rx_bytes_total {xdp_bytes}\n",
            if valkey_connected { 1 } else { 0 },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_render_contains_required_series() {
        let m = DaemonMetrics::new();
        m.record_evaluation(10, true);
        m.record_evaluation(20, false);
        m.record_pruned(2);
        m.record_xdp_rx(7, 700);
        let out = m.render_prometheus(5, 3, true);
        for series in [
            "edgeflag_evaluations_total 2",
            "edgeflag_l1_cache_hits_total 1",
            "edgeflag_l1_cache_misses_total 1",
            "edgeflag_wal_queue_depth 5",
            "edgeflag_active_websockets 3",
            "edgeflag_valkey_connected 1",
            "edgeflag_slow_consumers_pruned_total 2",
            "edgeflag_xdp_rx_packets_total 7",
            "edgeflag_xdp_rx_bytes_total 700",
        ] {
            assert!(out.contains(series), "missing series: {series}\n{out}");
        }
    }
}
