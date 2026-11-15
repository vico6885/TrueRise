//! Criterion baselines for the eval crate hot paths: fingerprint digesting
//! (every claim keys on it), ratchet tighten/verify, and conformance judging.
//! The CI `benchmark` job saves the `main` baseline and applies a 20%
//! regression gate from the second run onward.
#![allow(missing_docs)] // criterion_group! generates undocumented pub fns (repo idiom, cf. chain_baseline)

use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::PathBuf;

use criterion::{Criterion, criterion_group, criterion_main};
use truerise_eval::ratchet::Baseline;
use truerise_eval::{Fingerprint, Ratchet};
use truerise_sandbox::{CapabilityLevel, ConformanceReport, ProbeFact};

fn report_with(facts: usize) -> ConformanceReport {
    let mut fs = Vec::with_capacity(facts);
    for i in 0..facts {
        fs.push(ProbeFact {
            region: ["escape", "oversell", "residue"][i % 3].to_string(),
            probe: format!("probe-{i}"),
            passed: true,
            detail: "ok".to_string(),
        });
    }
    ConformanceReport {
        provider: "bench".to_string(),
        capability: CapabilityLevel::Full,
        declared_bits: 0,
        facts: fs,
        generated_at_dir: PathBuf::from("."),
    }
}

fn bench_fingerprint(c: &mut Criterion) {
    let fp = Fingerprint {
        model: "m*".to_string(),
        harness: "h*".to_string(),
        environment: "e*".to_string(),
        evaluator: "v*".to_string(),
    };
    c.bench_function("fingerprint/digest", |b| b.iter(|| black_box(&fp).digest()));
}

fn bench_ratchet(c: &mut Criterion) {
    let initial: Baseline = BTreeMap::from([("score".to_string(), 0.5)]);
    c.bench_function("ratchet/propose_tighten", |b| {
        b.iter(|| {
            let mut r = Ratchet::new(initial.clone());
            let tightened: Baseline = BTreeMap::from([("score".to_string(), 0.4)]);
            black_box(&mut r).propose(black_box(&tightened))
        })
    });
}

fn bench_judge(c: &mut Criterion) {
    let report = report_with(30);
    c.bench_function("judge/conformance_30_facts", |b| {
        b.iter(|| truerise_eval::judge(black_box(&report)))
    });
}

criterion_group!(benches, bench_fingerprint, bench_ratchet, bench_judge);
criterion_main!(benches);
