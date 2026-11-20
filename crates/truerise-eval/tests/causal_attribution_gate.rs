//! Formal attribution (CA5) behavioural tests, including the review-round
//! regressions: the cyclic attribution path (P1-1), declaration-order
//! independence of acyclicity (P2-7), and the witness-cap incompleteness
//! boundary (P1-2).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use truerise_eval::causal::attribution::{
    AttributionError, BinaryScm, CAUSAL_SEARCH_CAP, MAX_CONTINGENCY, VarDef,
};

fn var(name: &str, parents: Vec<usize>, func: impl Fn(&[bool]) -> bool + 'static) -> VarDef {
    VarDef {
        name: name.to_string(),
        parents,
        func: Box::new(func),
    }
}

fn cause_names(scm: &BinaryScm, effect: usize) -> Vec<String> {
    scm.causes_of(effect)
        .unwrap()
        .into_iter()
        .map(|c| c.cause_name)
        .collect()
}

/// The canonical preemption model (Halpern's formulation): ST = Suzy
/// throws, BT = Billy throws, SH = her rock hits, BH = his rock hits only
/// if NOT preempted, SG = the glass shatters on either hit.
fn suzy_billy() -> BinaryScm {
    BinaryScm::build(vec![
        var("ST", vec![], |_| true),
        var("BT", vec![], |_| true),
        var("SH", vec![0], |v| v[0]),
        var("BH", vec![1, 2], |v| v[1] && !v[2]),
        var("SG", vec![2, 3], |v| v[2] || v[3]),
    ])
    .unwrap()
}

#[test]
fn suzy_billy_preemption_has_suzy_as_cause_not_billy() {
    let scm = suzy_billy();
    let causes = scm.causes_of(4).unwrap();
    let names: Vec<&str> = causes.iter().map(|c| c.cause_name.as_str()).collect();
    // But-for alone fails for BOTH throws; the contingency witness (Billy's
    // rock held back) promotes ONLY Suzy's throw. Billy's throw must never
    // appear - promoting it is the classic bug.
    assert_eq!(names, vec!["ST", "SH"], "got {names:?}");
    for c in &causes {
        assert!((c.responsibility - 0.5).abs() < 1e-12);
        assert_eq!(
            c.witness,
            vec![(3usize, false)],
            "witness = hold Billy's rock"
        );
    }
}

#[test]
fn but_for_chain_grants_full_responsibility() {
    // ST -> SH -> SG, no Billy: both links are pure but-for causes.
    let scm = BinaryScm::build(vec![
        var("ST", vec![], |_| true),
        var("SH", vec![0], |v| v[0]),
        var("SG", vec![1], |v| v[1]),
    ])
    .unwrap();
    let causes = scm.causes_of(2).unwrap();
    let names: Vec<&str> = causes.iter().map(|c| c.cause_name.as_str()).collect();
    assert_eq!(names, vec!["ST", "SH"]);
    for c in &causes {
        assert!(
            c.witness.is_empty(),
            "no contingency needed for a clean chain"
        );
        assert!((c.responsibility - 1.0).abs() < 1e-12);
    }
}

#[test]
fn overdetermined_shatters_have_both_throws_as_causes() {
    // Two independent rocks, no preemption: each throw is a cause via the
    // contingency "hold the other rock".
    let scm = BinaryScm::build(vec![
        var("ST", vec![], |_| true),
        var("BT", vec![], |_| true),
        var("SH", vec![0], |v| v[0]),
        var("BH", vec![1], |v| v[1]),
        var("SG", vec![2, 3], |v| v[2] || v[3]),
    ])
    .unwrap();
    let causes = scm.causes_of(4).unwrap();
    let names: Vec<&str> = causes.iter().map(|c| c.cause_name.as_str()).collect();
    assert_eq!(names, vec!["ST", "BT", "SH", "BH"], "got {names:?}");
    assert!(
        causes
            .iter()
            .all(|c| (c.responsibility - 0.5).abs() < 1e-12)
    );
}

#[test]
fn disconnected_variable_is_never_a_cause() {
    // An irrelevant exogenous input must not be promoted by any contingency
    // gymnastics.
    let scm = BinaryScm::build(vec![
        var("IR", vec![], |_| true),
        var("ST", vec![], |_| true),
        var("SH", vec![1], |v| v[1]),
        var("SG", vec![2], |v| v[2]),
    ])
    .unwrap();
    let names = cause_names(&scm, 3);
    assert_eq!(names, vec!["ST", "SH"], "got {names:?}");
}

#[test]
fn ambiguous_or_impossible_actual_states_are_refused_at_build() {
    // A = !B, B = !A: two fixed points.
    let ambiguous = BinaryScm::build(vec![
        var("A", vec![1], |v| !v[1]),
        var("B", vec![0], |v| !v[0]),
    ]);
    assert!(matches!(
        ambiguous,
        Err(AttributionError::AmbiguousActualState { solutions: 2 })
    ));
    // A = !A: no fixed point.
    let impossible = BinaryScm::build(vec![var("A", vec![0], |v| !v[0])]);
    assert!(matches!(
        impossible,
        Err(AttributionError::NoConsistentActualState)
    ));
}

#[test]
fn cyclic_models_with_a_unique_solution_build_and_verify() {
    // A <- B <- C <- (constant): A = B, B = C, C = true. The parent chain
    // loops through index 0, forcing the cyclic path, and the solution is
    // unique.
    let scm = BinaryScm::build(vec![
        var("A", vec![1], |v| v[1]),
        var("B", vec![2], |v| v[2]),
        var("C", vec![], |_| true),
    ])
    .unwrap();
    assert_eq!(scm.actual(), &[true, true, true]);
}

/// Review P1-1 regression: the cyclic attribution path used to check the
/// ORIGINAL equation for intervened variables, making every flipped
/// intervention unsatisfiable and every cyclic model return zero causes.
/// An intervention REPLACES the equation; the model below must attribute
/// B and C as but-for causes of A.
#[test]
fn cyclic_model_yields_real_causes() {
    let scm = BinaryScm::build(vec![
        var("A", vec![1], |v| v[1]),
        var("B", vec![2], |v| v[2]),
        var("C", vec![], |_| true),
    ])
    .unwrap();
    let causes = scm.causes_of(0).unwrap();
    let names: Vec<&str> = causes.iter().map(|c| c.cause_name.as_str()).collect();
    assert_eq!(names, vec!["B", "C"], "got {names:?}");
    for c in &causes {
        assert!(c.witness.is_empty(), "pure but-for in this model");
        assert!((c.responsibility - 1.0).abs() < 1e-12);
    }
    // And transitively through the loop: B's only cause is C.
    assert_eq!(cause_names(&scm, 1), vec!["C"]);
}

/// Review P2-7 regression: acyclicity is a property of the DEPENDENCY
/// GRAPH, not the declaration order - a DAG declared in reverse index
/// order must take the linear topological path, not the cyclic one.
#[test]
fn reversed_index_dag_is_acyclic_and_attributable() {
    // Declaration order TOP, MID, ROOT; every parent index is GREATER than
    // the dependent's index. Kahn's algorithm must classify this acyclic.
    let scm = BinaryScm::build(vec![
        var("TOP", vec![1], |v| v[1]),
        var("MID", vec![2], |v| v[2]),
        var("ROOT", vec![], |_| true),
    ])
    .unwrap();
    assert_eq!(scm.actual(), &[true, true, true]);
    let names = cause_names(&scm, 0);
    assert_eq!(names, vec!["MID", "ROOT"], "got {names:?}");
}

/// Review P1-2 boundary pin: with MAX_CONTINGENCY = 4, a six-fold
/// overdetermination (every cause needs a 5-variable witness) finds NO
/// cause - the documented incompleteness, not a silent wrong answer and
/// not a refusal.
#[test]
fn witness_cap_pins_the_documented_incompleteness() {
    let mut defs = Vec::new();
    for i in 0..6 {
        defs.push(var(&format!("T{i}"), vec![], |_| true));
    }
    for i in 0..6u16 {
        defs.push(var(&format!("H{i}"), vec![usize::from(i)], move |v| {
            v[usize::from(i)]
        }));
    }
    let parents: Vec<usize> = (6..12).collect();
    defs.push(var("SG", parents.clone(), move |v| {
        parents.iter().any(|&p| v[p])
    }));
    let scm = BinaryScm::build(defs).unwrap();
    // Within the search contract (7 endogenous <= 12) but beyond the
    // witness cap: every true cause needs |W| = 5 > MAX_CONTINGENCY.
    assert!(usize::from(7u16) <= CAUSAL_SEARCH_CAP);
    assert_eq!(MAX_CONTINGENCY, 4);
    let causes = scm.causes_of(12).unwrap();
    assert!(
        causes.is_empty(),
        "with the cap at 4, six-fold overdetermination must report no cause - got {:?}",
        causes.iter().map(|c| &c.cause_name).collect::<Vec<_>>()
    );
}

#[test]
fn causal_search_beyond_compute_bound_is_refused() {
    // 13 endogenous variables in a chain: over the search contract.
    let mut defs = vec![var("X0", vec![], |_| true)];
    for i in 1..14 {
        let prev = i - 1;
        defs.push(var(&format!("X{i}"), vec![prev], move |v| v[prev]));
    }
    let scm = BinaryScm::build(defs).unwrap();
    assert!(matches!(
        scm.causes_of(13),
        Err(AttributionError::SearchTooLarge {
            endogenous: 13,
            cap: CAUSAL_SEARCH_CAP
        })
    ));
}

#[test]
fn effect_out_of_range_is_refused() {
    let scm = suzy_billy();
    assert!(matches!(
        scm.causes_of(99),
        Err(AttributionError::EffectOutOfRange { index: 99, vars: 5 })
    ));
}
