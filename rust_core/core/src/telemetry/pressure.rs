//! Deterministic pressure classification thresholds. Neither SRS nor TRS
//! pins exact numbers (SRS FR-MEM-001 requires the classification, not the
//! thresholds) — these are a documented judgment call, recorded in
//! docs/adr/0006-pressure-classification-thresholds.md.

use contracts::telemetry::{CpuPressure, MemoryPressure};

// Memory: primary signal is available/total. A value exactly at a boundary
// falls into the worse of the two adjacent tiers (see the boundary tests).
const MEM_AVAILABLE_RATIO_ELEVATED: f64 = 0.30;
const MEM_AVAILABLE_RATIO_HIGH: f64 = 0.15;
const MEM_AVAILABLE_RATIO_CRITICAL: f64 = 0.05;

// Memory: secondary signal, swap_used/swap_total. Can only raise the tier,
// never lower it.
const MEM_SWAP_RATIO_ELEVATED: f64 = 0.25;
const MEM_SWAP_RATIO_HIGH: f64 = 0.50;
const MEM_SWAP_RATIO_CRITICAL: f64 = 0.85;

// CPU: primary (and only) signal, aggregate utilization percent.
const CPU_UTIL_ELEVATED: f64 = 70.0;
const CPU_UTIL_HIGH: f64 = 85.0;
const CPU_UTIL_CRITICAL: f64 = 95.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MemoryPressureTier {
    Normal,
    Elevated,
    High,
    Critical,
}

impl From<MemoryPressureTier> for MemoryPressure {
    fn from(t: MemoryPressureTier) -> Self {
        match t {
            MemoryPressureTier::Normal => MemoryPressure::Normal,
            MemoryPressureTier::Elevated => MemoryPressure::Elevated,
            MemoryPressureTier::High => MemoryPressure::High,
            MemoryPressureTier::Critical => MemoryPressure::Critical,
        }
    }
}

fn available_ratio_tier(available_ratio: f64) -> MemoryPressureTier {
    if available_ratio <= MEM_AVAILABLE_RATIO_CRITICAL {
        MemoryPressureTier::Critical
    } else if available_ratio <= MEM_AVAILABLE_RATIO_HIGH {
        MemoryPressureTier::High
    } else if available_ratio <= MEM_AVAILABLE_RATIO_ELEVATED {
        MemoryPressureTier::Elevated
    } else {
        MemoryPressureTier::Normal
    }
}

fn swap_ratio_tier(swap_used_ratio: f64) -> MemoryPressureTier {
    if swap_used_ratio >= MEM_SWAP_RATIO_CRITICAL {
        MemoryPressureTier::Critical
    } else if swap_used_ratio >= MEM_SWAP_RATIO_HIGH {
        MemoryPressureTier::High
    } else if swap_used_ratio >= MEM_SWAP_RATIO_ELEVATED {
        MemoryPressureTier::Elevated
    } else {
        MemoryPressureTier::Normal
    }
}

/// `total_bytes`/`available_bytes` should be the host total or the resolved
/// cgroup ceiling, whichever the caller already determined. `swap` is
/// `None` when no swap is configured (the secondary signal is skipped).
pub fn classify_memory_pressure(
    total_bytes: u64,
    available_bytes: u64,
    swap_total_bytes: u64,
    swap_used_bytes: u64,
) -> MemoryPressure {
    let available_ratio = if total_bytes == 0 {
        1.0
    } else {
        available_bytes as f64 / total_bytes as f64
    };
    let mem_tier = available_ratio_tier(available_ratio);

    let final_tier = if swap_total_bytes == 0 {
        mem_tier
    } else {
        let swap_ratio = swap_used_bytes as f64 / swap_total_bytes as f64;
        let swap_tier = swap_ratio_tier(swap_ratio);
        mem_tier.max(swap_tier)
    };

    final_tier.into()
}

pub fn classify_cpu_pressure(aggregate_utilization_percent: f64) -> CpuPressure {
    if aggregate_utilization_percent >= CPU_UTIL_CRITICAL {
        CpuPressure::Critical
    } else if aggregate_utilization_percent >= CPU_UTIL_HIGH {
        CpuPressure::High
    } else if aggregate_utilization_percent >= CPU_UTIL_ELEVATED {
        CpuPressure::Elevated
    } else {
        CpuPressure::Normal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_boundaries() {
        assert_eq!(
            classify_memory_pressure(100, 31, 0, 0),
            MemoryPressure::Normal
        );
        assert_eq!(
            classify_memory_pressure(100, 30, 0, 0),
            MemoryPressure::Elevated
        );
        assert_eq!(
            classify_memory_pressure(100, 15, 0, 0),
            MemoryPressure::High
        );
        assert_eq!(
            classify_memory_pressure(100, 5, 0, 0),
            MemoryPressure::Critical
        );
        assert_eq!(
            classify_memory_pressure(100, 0, 0, 0),
            MemoryPressure::Critical
        );
    }

    #[test]
    fn swap_can_only_raise_the_tier() {
        // Plenty of memory available (Normal), but swap almost full ->
        // overall Critical.
        assert_eq!(
            classify_memory_pressure(100, 90, 100, 90),
            MemoryPressure::Critical
        );
        // Memory pressure Critical, no swap in use -> stays Critical (swap
        // never lowers it).
        assert_eq!(
            classify_memory_pressure(100, 1, 100, 0),
            MemoryPressure::Critical
        );
    }

    #[test]
    fn cpu_boundaries() {
        assert_eq!(classify_cpu_pressure(69.9), CpuPressure::Normal);
        assert_eq!(classify_cpu_pressure(70.0), CpuPressure::Elevated);
        assert_eq!(classify_cpu_pressure(85.0), CpuPressure::High);
        assert_eq!(classify_cpu_pressure(95.0), CpuPressure::Critical);
        assert_eq!(classify_cpu_pressure(100.0), CpuPressure::Critical);
    }
}
