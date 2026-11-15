#![allow(missing_docs)]
// Benches are test-like targets: fixture setup may unwrap/expect
// (CONTRIBUTING: "tests may"); the deny lints relax for this target only.
#![allow(clippy::expect_used, clippy::unwrap_used)]

//! Chain baseline benchmark (spec §8: benchmark gate applies from M1/M2
//! onward - this is the hot-path baseline: hash-chain append + verify).
//! The 20% regression comparison activates once the first saved baseline
//! exists (CI saves `main` baseline on default branch runs).

use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::json;
use std::hint::black_box;
use truerise_governance::HashChain;
use truerise_wire::Severity;

fn bench_append(c: &mut Criterion) {
    let dir = std::env::temp_dir().join("truerise_bench_chain");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bench.jsonl");
    let _ = std::fs::remove_file(&path);
    let chain = HashChain::open(&path).expect("chain opens");
    let mut group = c.benchmark_group("chain");
    group.throughput(criterion::Throughput::Elements(1));
    group.bench_function("append", |b| {
        b.iter(|| {
            let i = black_box(0u64);
            chain
                .append(
                    "bench.event",
                    Severity::Info,
                    &json!({"i": i, "payload": "benchmark row for the append hot path"}),
                    None,
                )
                .expect("bench append");
        })
    });
    group.bench_function("verify_full", |b| {
        b.iter(|| chain.verify().expect("chain intact"))
    });
    group.finish();
    let _ = std::fs::remove_dir_all(&dir);
}

criterion_group!(benches, bench_append);
criterion_main!(benches);
