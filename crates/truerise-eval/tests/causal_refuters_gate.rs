//! Refuter battery behavioural tests (CA3 + review family B), moved out
//! of the implementation file for the file-length gate. Includes the
//! negative-control refuter and the review regressions present HERE: the
//! relative pivot threshold and the valid-permutation counting on the
//! singular-refit path. (The overflow regressions live in the permutation
//! and ICP module tests.)

#![allow(clippy::expect_used, clippy::unwrap_used)]

use truerise_eval::causal::refute::{
    Design, RefuteError, confounder_injection, estimate_effect, negative_control, ols_fit,
    placebo_test, subset_stability,
};
use truerise_eval::causal::rng::SplitMix64;
/// Deterministic study: t alternates 0/1, y = 2t + seeded noise.
fn noisy_design(n: usize, noise_seed: u64, effect: f64) -> Design {
    let mut rng = SplitMix64::new(noise_seed);
    let treatment: Vec<f64> = (0..n).map(|i| (i % 2) as f64).collect();
    let outcome: Vec<f64> = treatment
        .iter()
        .map(|&t| effect * t + (rng.next_f64() - 0.5) * 0.2)
        .collect();
    Design {
        treatment,
        covariates: Vec::new(),
        outcome,
    }
}

#[test]
fn exact_linear_relation_is_recovered_bit_tight() {
    let design = noisy_design(40, 1, 2.0);
    // Overwrite with the exact relation: y = 2t, zero noise.
    let outcome: Vec<f64> = design.treatment.iter().map(|&t| 2.0 * t).collect();
    let design = Design { outcome, ..design };
    let effect = estimate_effect(&design).unwrap();
    assert!((effect - 2.0).abs() < 1e-9, "was {effect}");
}

#[test]
fn placebo_challenge_passes_a_real_effect_and_flags_noise() {
    // Real effect: permuted labels must NOT reach the observed contrast.
    let real = noisy_design(60, 3, 2.0);
    let report = placebo_test(&real, 500, 7).unwrap();
    assert!(
        report.placebo_p < 0.05,
        "real effect looked placebo-like: {}",
        report.placebo_p
    );
    // Pure noise: the observed coefficient is just another placebo draw
    // - its percentile within the placebo distribution is unremarkable.
    // (A noise coefficient sits mid-distribution by construction; the
    // deterministic seed pins this at ~0.17, comfortably un-special.)
    let noise = noisy_design(60, 3, 0.0);
    let report = placebo_test(&noise, 500, 7).unwrap();
    assert!(
        report.placebo_p > 0.1,
        "noise looked special: {}",
        report.placebo_p
    );
    assert!(
        report.placebo_p < 0.6,
        "noise should not look IMPOSSIBLE either: {}",
        report.placebo_p
    );
}

#[test]
fn stability_interval_covers_the_true_effect() {
    let design = noisy_design(80, 5, 2.0);
    let report = subset_stability(&design, 400, 9).unwrap();
    assert!(
        report.ci_lo <= 2.0 && 2.0 <= report.ci_hi,
        "interval [{}, {}] missed the truth",
        report.ci_lo,
        report.ci_hi
    );
    assert!(report.ci_hi > report.ci_lo, "noise must open the interval");
}

#[test]
fn zero_loading_injection_is_the_identity() {
    let design = noisy_design(50, 11, 2.0);
    let report = confounder_injection(&design, 0.0, 0.0, 20, 13).unwrap();
    assert!((report.shifted_min - report.base).abs() < 1e-12);
    assert!((report.shifted_max - report.base).abs() < 1e-12);
}

#[test]
fn strong_injection_loadings_move_the_estimate_far() {
    // Noiseless design (y = 2t exactly) and balanced factor draws make
    // the shift analytic: with loadings (0.4, -4) the re-estimate is
    // 2*0.25/0.41 - 4*(cov(u,t) + 0.4)/0.41, which sits near -2.7 with
    // every draw. Anything under base - 1.0 proves the refuter reacts
    // to loadings in the direction the outcome channel implies.
    let design = noisy_design(200, 17, 2.0);
    let outcome: Vec<f64> = design.treatment.iter().map(|&t| 2.0 * t).collect();
    let clean = Design { outcome, ..design };
    let report = confounder_injection(&clean, 0.4, -4.0, 50, 19).unwrap();
    assert!(
        report.shifted_max < report.base - 1.0,
        "base {}, max shifted {} - injection did not bite",
        report.base,
        report.shifted_max
    );
    assert!(report.shifted_min <= report.shifted_max);
}

#[test]
fn singular_and_malformed_designs_are_refused() {
    // Duplicated column -> singular.
    let design = Design {
        treatment: vec![0.0, 1.0, 0.0, 1.0, 1.0],
        covariates: vec![vec![0.0, 1.0, 0.0, 1.0, 1.0]],
        outcome: vec![1.0, 2.0, 1.0, 2.0, 2.0],
    };
    assert!(matches!(
        estimate_effect(&design),
        Err(RefuteError::SingularDesign)
    ));
    // Row misalignment.
    let design = Design {
        treatment: vec![0.0, 1.0],
        covariates: Vec::new(),
        outcome: vec![1.0, 2.0, 3.0],
    };
    assert!(matches!(
        estimate_effect(&design),
        Err(RefuteError::LengthMismatch { .. })
    ));
    // NaN.
    let design = Design {
        treatment: vec![0.0, 1.0, f64::NAN, 1.0, 0.0],
        covariates: Vec::new(),
        outcome: vec![1.0, 2.0, 3.0, 4.0, 5.0],
    };
    assert!(matches!(
        estimate_effect(&design),
        Err(RefuteError::NotFinite { .. })
    ));
    // Underdetermined: 3 rows, treatment + 2 covariates = 3 columns.
    let design = Design {
        treatment: vec![0.0, 1.0, 0.0],
        covariates: vec![vec![1.0, 0.0, 1.0], vec![0.0, 1.0, 1.0]],
        outcome: vec![1.0, 2.0, 3.0],
    };
    assert!(matches!(
        estimate_effect(&design),
        Err(RefuteError::TooFewRows { .. })
    ));
}

/// Review family B: a negative-control outcome the treatment cannot
/// causally move must stay still; if the treatment "moves" it, the
/// confounding leaked and the primary estimate is flagged.
#[test]
fn negative_control_flags_leaked_confounding() {
    // Confounded construction: u drives BOTH the treatment and the
    // negative-control outcome (N = 3u), while the primary outcome is
    // clean (y = 2t with t balanced).
    let n = 60;
    let mut rng = SplitMix64::new(97);
    let treatment: Vec<f64> = (0..n).map(|i| (i % 2) as f64).collect();
    let mut nc = Vec::with_capacity(n);
    for &t in &treatment {
        let u = if t == 1.0 && rng.next_f64() < 0.9 {
            1.0
        } else if t == 0.0 && rng.next_f64() < 0.9 {
            0.0
        } else {
            1.0 - t
        };
        nc.push(3.0 * u + (rng.next_f64() - 0.5) * 0.1);
    }
    let design = Design {
        treatment: treatment.clone(),
        covariates: Vec::new(),
        outcome: treatment.iter().map(|t| 2.0 * t).collect(),
    };
    let report = negative_control(&design, &nc, 300, 5).unwrap();
    assert!(
        report.flags_residual_confounding,
        "a 3-unit leak into the negative control must flag: effect {}, p {}",
        report.control_effect, report.control_p
    );
    assert!(report.control_p < 0.05, "p was {}", report.control_p);
}

#[test]
fn clean_negative_control_stays_quiet() {
    // N independent of the balanced treatment: no leak, no flag.
    let n = 60;
    let mut rng = SplitMix64::new(99);
    let treatment: Vec<f64> = (0..n).map(|i| (i % 2) as f64).collect();
    let nc: Vec<f64> = (0..n).map(|_| (rng.next_f64() - 0.5) * 0.2).collect();
    let design = Design {
        treatment: treatment.clone(),
        covariates: Vec::new(),
        outcome: treatment.iter().map(|t| 2.0 * t).collect(),
    };
    let report = negative_control(&design, &nc, 300, 7).unwrap();
    assert!(
        !report.flags_residual_confounding,
        "pure noise in the control outcome flagged a leak: p {}",
        report.control_p
    );
    assert!(report.control_p > 0.1, "p was {}", report.control_p);
}

/// Review P1-2 regression: permuted refits that collide the treatment
/// with a covariate column are SINGULAR and skipped - the report must
/// carry the VALID count, not the requested one. Fixture: t and c share
/// the same multiset (three ones) in different arrangements, so ~5% of
/// permutations land exactly on the collinear pattern.
#[test]
fn singular_refits_shrink_the_reported_valid_count() {
    let design = Design {
        treatment: vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
        covariates: vec![vec![0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0]],
        outcome: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
    };
    let report = placebo_test(&design, 200, 3).unwrap();
    assert!(
        report.permutations < 200,
        "colliding permutations must be skipped, valid = {}",
        report.permutations
    );
    assert!(report.permutations > 0, "most refits are estimable");
    // And the negative-control refuter refuses a zero-draw call outright
    // (review P1-1): no evidence, no verdict.
    let clean = Design {
        treatment: vec![0.0, 1.0, 0.0, 1.0, 1.0],
        covariates: Vec::new(),
        outcome: vec![1.0, 2.0, 1.0, 2.0, 2.0],
    };
    assert!(matches!(
        negative_control(&clean, &[1.0, 2.0, 3.0, 4.0, 5.0], 0, 1),
        Err(RefuteError::BadDrawCount { count: 0 })
    ));
}

#[test]
fn negative_control_misaligned_rows_are_refused() {
    let design = Design {
        treatment: vec![0.0, 1.0, 0.0, 1.0, 1.0],
        covariates: Vec::new(),
        outcome: vec![1.0, 2.0, 1.0, 2.0, 2.0],
    };
    assert!(matches!(
        negative_control(&design, &[1.0, 2.0, 3.0], 10, 1),
        Err(RefuteError::LengthMismatch { .. })
    ));
}

#[test]
fn ols_fit_solves_a_known_two_column_system() {
    // y = 1 + 3x exactly at three points.
    let ones = [1.0, 1.0, 1.0];
    let x = [0.0, 1.0, 2.0];
    let y = [1.0, 4.0, 7.0];
    let beta = ols_fit(&[&ones, &x], &y).unwrap();
    assert!((beta[0] - 1.0).abs() < 1e-9);
    assert!((beta[1] - 3.0).abs() < 1e-9);
    // Rank-deficient: two identical columns.
    let beta = ols_fit(&[&x, &x], &y);
    assert!(beta.is_none());
}

/// Review P2-5 regression: the pivot threshold is RELATIVE to the
/// matrix scale - a uniformly tiny but perfectly conditioned system
/// (all columns at 1e-8) must solve, not be refused as singular.
#[test]
fn uniformly_tiny_well_posed_systems_solve() {
    let x = [1e-8, 2e-8, 3e-8];
    let y = [2e-8, 4e-8, 6e-8]; // y = 2x exactly
    let beta = ols_fit(&[&x], &y).unwrap();
    assert!((beta[0] - 2.0).abs() < 1e-9, "was {}", beta[0]);
    // The all-zero normal matrix stays singular.
    let zero = [0.0, 0.0, 0.0];
    assert!(ols_fit(&[&zero], &y).is_none());
}
