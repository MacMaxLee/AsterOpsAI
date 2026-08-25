use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ai_ops_core::telemetry_macos::context::SampleContext;
use ai_ops_core::telemetry_macos::cpu::parse_cpu_snapshot;
use ai_ops_core::telemetry_macos::memory::parse_memory_snapshot;
use ai_ops_core::telemetry_macos::network::parse_network_snapshot;
use ai_ops_core::telemetry_macos::process::parse_process_snapshot;
use ai_ops_core::telemetry_macos::storage::parse_storage_snapshot;
use std::time::Duration;

fn bench_cpu_snapshot(c: &mut Criterion) {
    let ctx = SampleContext {
        now: chrono::Utc::now(),
        elapsed: Duration::from_secs(1),
        configured_interval: Duration::from_secs(1),
    };

    // First sample (no prior state)
    c.bench_function("cpu_snapshot_first", |b| {
        b.iter(|| {
            let _ = parse_cpu_snapshot(black_box(&ctx), None);
        });
    });

    // Warm up with first sample to get prior state
    let (_, prev) = parse_cpu_snapshot(&ctx, None).expect("CPU snapshot failed");
    std::thread::sleep(Duration::from_millis(100));

    // Subsequent sample (with prior state for delta calculations)
    c.bench_function("cpu_snapshot_with_state", |b| {
        b.iter(|| {
            let _ = parse_cpu_snapshot(black_box(&ctx), Some(black_box(&prev)));
        });
    });
}

fn bench_memory_snapshot(c: &mut Criterion) {
    let ctx = SampleContext {
        now: chrono::Utc::now(),
        elapsed: Duration::from_secs(1),
        configured_interval: Duration::from_secs(1),
    };

    c.bench_function("memory_snapshot", |b| {
        b.iter(|| {
            let _ = parse_memory_snapshot(black_box(&ctx));
        });
    });
}

fn bench_storage_snapshot(c: &mut Criterion) {
    let ctx = SampleContext {
        now: chrono::Utc::now(),
        elapsed: Duration::from_secs(1),
        configured_interval: Duration::from_secs(1),
    };

    c.bench_function("storage_snapshot", |b| {
        b.iter(|| {
            let _ = parse_storage_snapshot(black_box(&ctx));
        });
    });
}

fn bench_network_snapshot(c: &mut Criterion) {
    let ctx = SampleContext {
        now: chrono::Utc::now(),
        elapsed: Duration::from_secs(1),
        configured_interval: Duration::from_secs(1),
    };

    // First sample (no prior state)
    c.bench_function("network_snapshot_first", |b| {
        b.iter(|| {
            let _ = parse_network_snapshot(black_box(&ctx), None);
        });
    });

    // Warm up with first sample to get prior state
    let (_, prev) = parse_network_snapshot(&ctx, None).expect("Network snapshot failed");
    std::thread::sleep(Duration::from_millis(100));

    // Subsequent sample (with prior state for rate calculations)
    c.bench_function("network_snapshot_with_state", |b| {
        b.iter(|| {
            let _ = parse_network_snapshot(black_box(&ctx), Some(black_box(&prev)));
        });
    });
}

fn bench_process_snapshot(c: &mut Criterion) {
    let ctx = SampleContext {
        now: chrono::Utc::now(),
        elapsed: Duration::from_secs(1),
        configured_interval: Duration::from_secs(1),
    };

    // First sample (no prior state)
    c.bench_function("process_snapshot_first", |b| {
        b.iter(|| {
            let _ = parse_process_snapshot(black_box(&ctx), None);
        });
    });

    // Warm up with first sample to get prior state
    let (_, prev) = parse_process_snapshot(&ctx, None).expect("Process snapshot failed");
    std::thread::sleep(Duration::from_millis(100));

    // Subsequent sample (with prior state for CPU% calculations)
    c.bench_function("process_snapshot_with_state", |b| {
        b.iter(|| {
            let _ = parse_process_snapshot(black_box(&ctx), Some(black_box(&prev)));
        });
    });
}

fn bench_full_telemetry_cycle(c: &mut Criterion) {
    let ctx = SampleContext {
        now: chrono::Utc::now(),
        elapsed: Duration::from_secs(1),
        configured_interval: Duration::from_secs(1),
    };

    // Warm up: Get initial state for functions that need it
    let (_, cpu_prev) = parse_cpu_snapshot(&ctx, None).expect("CPU warmup failed");
    let (_, net_prev) = parse_network_snapshot(&ctx, None).expect("Network warmup failed");
    let (_, proc_prev) = parse_process_snapshot(&ctx, None).expect("Process warmup failed");
    std::thread::sleep(Duration::from_millis(100));

    // Full cycle: All 5 telemetry functions in sequence (realistic production scenario)
    c.bench_function("full_telemetry_cycle", |b| {
        b.iter(|| {
            // CPU with state
            let _ = parse_cpu_snapshot(black_box(&ctx), Some(black_box(&cpu_prev)));

            // Memory (no state needed)
            let _ = parse_memory_snapshot(black_box(&ctx));

            // Storage (no state needed)
            let _ = parse_storage_snapshot(black_box(&ctx));

            // Network with state
            let _ = parse_network_snapshot(black_box(&ctx), Some(black_box(&net_prev)));

            // Process with state (likely the slowest operation)
            let _ = parse_process_snapshot(black_box(&ctx), Some(black_box(&proc_prev)));
        });
    });
}

criterion_group!(
    benches,
    bench_cpu_snapshot,
    bench_memory_snapshot,
    bench_storage_snapshot,
    bench_network_snapshot,
    bench_process_snapshot,
    bench_full_telemetry_cycle
);
criterion_main!(benches);
