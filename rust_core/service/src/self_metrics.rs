//! Tracks this process's own CPU%/RSS, exposed on `/api/v1/health` (TRS §40).
//! Sampling happens on a background interval so the health handler never
//! blocks on a syscall per request.

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::SelfMetricValue;
use platform::PlatformAdapter;
use tokio::sync::RwLock;

const SAMPLE_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct SelfMetricsSnapshot {
    pub cpu_percent: SelfMetricValue<f64>,
    pub rss_bytes: SelfMetricValue<u64>,
}

impl SelfMetricsSnapshot {
    fn unavailable(reason: String) -> Self {
        Self {
            cpu_percent: SelfMetricValue::Unavailable {
                reason: reason.clone(),
            },
            rss_bytes: SelfMetricValue::Unavailable { reason },
        }
    }
}

/// Computes CPU% from the delta between consecutive samples on a monotonic
/// clock — never from a single reading (TRS §7's rate-calculation
/// discipline, applied here to the service's own resource usage).
struct SelfMetricsSampler {
    platform: Arc<dyn PlatformAdapter>,
    last: Option<(Instant, Duration)>,
}

impl SelfMetricsSampler {
    fn new(platform: Arc<dyn PlatformAdapter>) -> Self {
        Self {
            platform,
            last: None,
        }
    }

    fn sample(&mut self) -> SelfMetricsSnapshot {
        let metrics = match self.platform.self_process_metrics() {
            Ok(metrics) => metrics,
            Err(err) => {
                self.last = None;
                return SelfMetricsSnapshot::unavailable(err.to_string());
            }
        };

        let now = Instant::now();
        let rss_bytes = SelfMetricValue::Supported {
            value: metrics.rss_bytes,
        };

        let cpu_percent = match self.last {
            Some((prev_instant, prev_cpu_time)) => {
                let wall_elapsed = now.duration_since(prev_instant);
                if wall_elapsed.is_zero() || metrics.cpu_time < prev_cpu_time {
                    SelfMetricValue::Unavailable {
                        reason: "clock or counter anomaly between samples".to_string(),
                    }
                } else {
                    let cpu_elapsed = metrics.cpu_time - prev_cpu_time;
                    let percent = cpu_elapsed.as_secs_f64() / wall_elapsed.as_secs_f64() * 100.0;
                    SelfMetricValue::Supported { value: percent }
                }
            }
            None => SelfMetricValue::Unavailable {
                reason: "insufficient samples yet".to_string(),
            },
        };

        self.last = Some((now, metrics.cpu_time));
        SelfMetricsSnapshot {
            cpu_percent,
            rss_bytes,
        }
    }
}

/// Takes an immediate first sample (so `/health` has data right away), then
/// spawns a background task that keeps refreshing it every [`SAMPLE_INTERVAL`].
pub fn spawn(platform: Arc<dyn PlatformAdapter>) -> Arc<RwLock<SelfMetricsSnapshot>> {
    let mut sampler = SelfMetricsSampler::new(platform);
    let snapshot = Arc::new(RwLock::new(sampler.sample()));

    let shared = snapshot.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(SAMPLE_INTERVAL);
        interval.tick().await; // first tick fires immediately; we already sampled above
        loop {
            interval.tick().await;
            let value = sampler.sample();
            *shared.write().await = value;
        }
    });

    snapshot
}
