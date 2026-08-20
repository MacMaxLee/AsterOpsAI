//! Benchmark statistical methodology (unit U9, SRS §21 FR-BENCH-001..003,
//! TRS §34-35).
//!
//! **Hard boundary (SRS FR-BENCH-001/002)**: no verdict is ever produced
//! from a single point-in-time measurement — every comparison is between
//! two real sampled distributions, and the honest default is
//! [`verdict::BenchmarkVerdict::Inconclusive`], never silently upgraded.
//!
//! **Hard boundary**: nothing here depends on `core::ai` or
//! `core::analysis`'s own window-level scoring — a benchmark verdict is
//! its own, statistically-grounded concept (see `verdict.rs`'s own doc
//! comment for why `HostVerdict.score` can't serve as the benchmarked
//! metric), matching the same "no AI in the decision path" precedent
//! `core::analysis`/`core::ai`/`core::policy` already established.
//!
//! **Scope note**: `run_benchmark` drives U7/U8's executor pipeline
//! unmodified — it adds no new action-execution machinery, only the
//! measure-before/measure-after wrapper, the verdict, and TRS §35's
//! rollback triggers.

pub mod config;
pub mod confounders;
pub mod error;
pub mod run;
pub mod sampler;
pub mod stats;
pub mod verdict;

pub use config::BenchmarkConfig;
pub use confounders::{Confounders, SampleGap};
pub use error::BenchmarkError;
pub use run::{run_benchmark, BenchmarkOutcome, BenchmarkPipeline, BenchmarkRunRequest};
pub use sampler::{MetricDirection, MetricSampler};
pub use verdict::{resolve, BenchmarkVerdict, BASELINE_UNSTABLE};

#[cfg(target_os = "linux")]
pub use sampler::HostCpuUtilizationSampler;
