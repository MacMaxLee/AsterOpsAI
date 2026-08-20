//! Shared test-only helpers for the benchmark orchestration tests.
//! Integration tests are already test-only code; the workspace's
//! unwrap/expect deny targets production code paths, not test files.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;

use ai_ops_core::benchmark::{BenchmarkError, MetricSampler};

/// Returns a pre-scripted sequence of values, one per call; once
/// exhausted, further calls repeat the final value forever (so a script
/// only needs to name exactly the values it cares about, not predict the
/// orchestrator's exact poll count). Real wall-clock waiting still happens
/// in `run_benchmark`'s own polling loop — only the *measured value*
/// itself is deterministic, per docs/adr/0014's testing-strategy
/// rationale: real pipeline wiring (real spawned process, real action
/// execution, real waiting), deterministic measured values, so verdict
/// direction isn't a bet on this sandbox's CPU scheduler.
pub struct ScriptedSampler {
    values: Vec<f64>,
    index: Mutex<usize>,
}

impl ScriptedSampler {
    pub fn new(values: Vec<f64>) -> Self {
        assert!(
            !values.is_empty(),
            "a scripted sampler needs at least one value"
        );
        Self {
            values,
            index: Mutex::new(0),
        }
    }
}

impl MetricSampler for ScriptedSampler {
    fn sample(&self) -> Result<f64, BenchmarkError> {
        let mut idx = self
            .index
            .lock()
            .map_err(|_| BenchmarkError::Sample("poisoned lock".to_string()))?;
        let i = (*idx).min(self.values.len() - 1);
        let value = self.values[i];
        if *idx < self.values.len() {
            *idx += 1;
        }
        Ok(value)
    }
}

/// A more robust scripted sampler for `run_benchmark`'s two-window shape:
/// rather than guessing how many calls the baseline window will make
/// before hardcoding a fixed prefix length (fragile — `collect_window`
/// only consumes as many samples as its own stability/count conditions
/// need, which isn't precisely predictable from outside), `is_post_change`
/// checks *real* live state (e.g. whether the target process's priority
/// has actually changed yet) to decide which scripted sequence to draw
/// from. The phase transition is real; only the magnitude within each
/// phase is deterministic.
pub struct PhaseAwareScriptedSampler<F: Fn() -> bool + Send + Sync> {
    is_post_change: F,
    baseline_values: Vec<f64>,
    post_change_values: Vec<f64>,
    baseline_index: Mutex<usize>,
    post_change_index: Mutex<usize>,
}

impl<F: Fn() -> bool + Send + Sync> PhaseAwareScriptedSampler<F> {
    pub fn new(is_post_change: F, baseline_values: Vec<f64>, post_change_values: Vec<f64>) -> Self {
        assert!(!baseline_values.is_empty() && !post_change_values.is_empty());
        Self {
            is_post_change,
            baseline_values,
            post_change_values,
            baseline_index: Mutex::new(0),
            post_change_index: Mutex::new(0),
        }
    }

    fn next_from(values: &[f64], index: &Mutex<usize>) -> Result<f64, BenchmarkError> {
        let mut idx = index
            .lock()
            .map_err(|_| BenchmarkError::Sample("poisoned lock".to_string()))?;
        let i = (*idx).min(values.len() - 1);
        let value = values[i];
        if *idx < values.len() {
            *idx += 1;
        }
        Ok(value)
    }
}

impl<F: Fn() -> bool + Send + Sync> MetricSampler for PhaseAwareScriptedSampler<F> {
    fn sample(&self) -> Result<f64, BenchmarkError> {
        if (self.is_post_change)() {
            Self::next_from(&self.post_change_values, &self.post_change_index)
        } else {
            Self::next_from(&self.baseline_values, &self.baseline_index)
        }
    }
}
