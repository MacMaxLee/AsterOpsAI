//! Requirement 4's gap/reset discipline, implemented once and reused by
//! every counter-based parser: `elapsed > 2x configured_interval` emits
//! `SampleGap`; a decreasing counter emits `CounterReset`; otherwise a plain
//! rate — never a negative one.

use contracts::telemetry::MetricValue;

use super::context::SampleContext;

pub(crate) fn rate_per_second(prev: u64, curr: u64, ctx: &SampleContext) -> MetricValue<f64> {
    if ctx.elapsed > ctx.configured_interval * 2 {
        return MetricValue::SampleGap {
            reason: format!(
                "elapsed {:?} exceeds 2x sample interval {:?}",
                ctx.elapsed, ctx.configured_interval
            ),
        };
    }
    if curr < prev {
        return MetricValue::CounterReset {
            reason: format!("counter decreased from {prev} to {curr}"),
        };
    }
    if ctx.elapsed.is_zero() {
        return MetricValue::Unavailable {
            reason: "zero elapsed time between samples".to_string(),
        };
    }
    MetricValue::Supported {
        value: (curr - prev) as f64 / ctx.elapsed.as_secs_f64(),
    }
}

/// Same discipline, specialised for a jiffies-idle-over-jiffies-total ratio
/// (CPU utilization). Matches `mpstat`'s `%idle` convention: `idle_*` must be
/// the raw idle jiffy field alone, not idle+iowait.
pub(crate) fn utilization_percent(
    idle_prev: u64,
    idle_curr: u64,
    total_prev: u64,
    total_curr: u64,
    ctx: &SampleContext,
) -> MetricValue<f64> {
    if ctx.elapsed > ctx.configured_interval * 2 {
        return MetricValue::SampleGap {
            reason: format!(
                "elapsed {:?} exceeds 2x sample interval {:?}",
                ctx.elapsed, ctx.configured_interval
            ),
        };
    }
    if total_curr < total_prev || idle_curr < idle_prev {
        return MetricValue::CounterReset {
            reason: "cpu jiffy counter decreased".to_string(),
        };
    }
    let total_delta = total_curr - total_prev;
    if total_delta == 0 {
        return MetricValue::Unavailable {
            reason: "no elapsed CPU ticks".to_string(),
        };
    }
    let idle_delta = idle_curr - idle_prev;
    let pct = 100.0 * (1.0 - idle_delta as f64 / total_delta as f64);
    MetricValue::Supported {
        value: pct.clamp(0.0, 100.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ctx(elapsed_secs: u64) -> SampleContext {
        SampleContext {
            now: chrono::Utc::now(),
            elapsed: Duration::from_secs(elapsed_secs),
            configured_interval: Duration::from_secs(1),
        }
    }

    #[test]
    fn plain_rate() {
        let v = rate_per_second(100, 200, &ctx(1));
        assert!(matches!(v, MetricValue::Supported { value } if (value - 100.0).abs() < 1e-9));
    }

    #[test]
    fn counter_reset_detected() {
        let v = rate_per_second(200, 100, &ctx(1));
        assert!(matches!(v, MetricValue::CounterReset { .. }));
    }

    #[test]
    fn sample_gap_detected() {
        let v = rate_per_second(100, 200, &ctx(10));
        assert!(matches!(v, MetricValue::SampleGap { .. }));
    }

    #[test]
    fn utilization_clamped_and_matches_idle_only() {
        // total delta 1000, idle delta 0 -> 100% busy
        let v = utilization_percent(0, 0, 0, 1000, &ctx(1));
        assert!(matches!(v, MetricValue::Supported { value } if (value - 100.0).abs() < 1e-9));
    }
}
