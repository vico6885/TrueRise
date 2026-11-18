//! Derivable-claim gate (G25): whatever is COMPUTABLE in a claim is
//! RECOMPUTED, never judged.
//!
//! Boundary (docs/limits.md): this gate refuses UNSUPPORTED or CONTRADICTED
//! claims in closed form - it does not and cannot certify open-world truth.
//! The decidable relations:
//! 1. **traceable**: a cited number must equal an artifact field that can
//!    be located (an unlocatable field is `unsupported`, not an error);
//! 2. **recomputable**: arithmetic over artifact fields (sum / difference /
//!    count) is recomputed exactly;
//! 3. **non-negotiable verdicts**: `contradicted` BLOCKS promotion (it is
//!    a refusal, never a downgrade); only `unsupported` can be remedied by
//!    adding evidence.

use serde_json::Value;

/// What kind of derivable claim is being checked (v1 kinds: the ones a
/// deterministic engine can decide exactly).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimKind {
    /// "The cited number equals artifact field X" (trace-to-field).
    FieldReference,
    /// "operands sum to the cited number" (all operands traced first).
    Sum,
    /// "b - a equals the cited number" (both operands traced first).
    Difference,
    /// "the array at X has exactly n items" (the count IS recomputable).
    Count,
}

/// One derivable claim: what is asserted and where the pieces live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivableClaim {
    /// The kind (decides the recomputation rule).
    pub kind: ClaimKind,
    /// Dotted field paths into the artifact (operands / cited values).
    pub fields: Vec<String>,
    /// The asserted value (what the claim says the result is).
    pub asserted: Value,
}

/// The verdict of a derivable-claim check. NOT three kinds of the same
/// thing: `contradicted` and `unsupported` have DIFFERENT consequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivableVerdict {
    /// Recomputed and matching - the claim carries support.
    Supported,
    /// Recomputed and MISMATCHING - the claim is refused, not downgraded.
    Contradicted {
        /// What was asserted vs what the artifact actually holds.
        reason: String,
    },
    /// The cited field could not be located - the claim has no support
    /// (it may be remedied by adding evidence; a contradiction cannot).
    Unsupported {
        /// The field that could not be traced.
        field: String,
    },
}

impl DerivableVerdict {
    /// The enforcement rule (G25's teeth): a contradiction BLOCKS
    /// promotion; an unsupported claim blocks AUTO-promotion too (both are
    /// refusals - they differ in remedy, not in outcome).
    #[must_use]
    pub fn blocks_promotion(&self) -> bool {
        !matches!(self, Self::Supported)
    }
}

/// Look up a dotted field path in a JSON artifact value.
fn field_lookup<'a>(artifact: &'a Value, dotted: &str) -> Option<&'a Value> {
    let mut current = artifact;
    for segment in dotted.split('.').filter(|s| !s.is_empty()) {
        current = current.get(segment)?;
    }
    Some(current)
}

fn as_number(value: &Value) -> Option<f64> {
    value.as_f64()
}

/// Check one derivable claim against an artifact. Deterministic by
/// construction: same artifact + same claim -> same verdict (offline
/// re-runnable; floats compared exactly - the float rule lives in the
/// claim data, not in this engine).
pub fn check(claim: &DerivableClaim, artifact: &Value) -> DerivableVerdict {
    match claim.kind {
        ClaimKind::FieldReference => {
            let Some(field) = claim.fields.first() else {
                return DerivableVerdict::Unsupported {
                    field: "(no field cited)".to_string(),
                };
            };
            match field_lookup(artifact, field) {
                Some(actual) if actual == &claim.asserted => DerivableVerdict::Supported,
                Some(actual) => DerivableVerdict::Contradicted {
                    reason: format!(
                        "field {field} = {actual}, the claim asserts {}",
                        claim.asserted
                    ),
                },
                None => DerivableVerdict::Unsupported {
                    field: field.clone(),
                },
            }
        }
        ClaimKind::Sum => {
            let mut total = 0.0f64;
            for field in &claim.fields {
                match field_lookup(artifact, field).and_then(as_number) {
                    Some(n) => total += n,
                    None => {
                        return DerivableVerdict::Unsupported {
                            field: field.clone(),
                        };
                    }
                }
            }
            // Review fix (P2): integer claims compare exactly; floats
            // compare exactly per the module contract - an absolute EPSILON
            // is a 1.0-scale ULP, wrong both for huge sums (false
            // Contradicted) and for accumulated rounding (false pass).
            let integer_match = claim
                .asserted
                .as_i64()
                .map(|asserted| total.fract() == 0.0 && total == asserted as f64)
                .unwrap_or(false);
            match claim.asserted.as_f64() {
                Some(asserted) if integer_match || total == asserted => DerivableVerdict::Supported,
                Some(asserted) => DerivableVerdict::Contradicted {
                    reason: format!(
                        "sum of {:?} = {total}, the claim asserts {asserted}",
                        claim.fields
                    ),
                },
                None => DerivableVerdict::Unsupported {
                    field: "(asserted value is not a number)".to_string(),
                },
            }
        }
        ClaimKind::Difference => {
            if claim.fields.len() != 2 {
                return DerivableVerdict::Unsupported {
                    field: "(difference needs exactly two operands)".to_string(),
                };
            }
            let a = field_lookup(artifact, &claim.fields[0]).and_then(as_number);
            let b = field_lookup(artifact, &claim.fields[1]).and_then(as_number);
            match (a, b) {
                (Some(a), Some(b)) => {
                    let diff = b - a;
                    match claim.asserted.as_f64() {
                        Some(asserted) if diff == asserted => DerivableVerdict::Supported,
                        Some(asserted) => DerivableVerdict::Contradicted {
                            reason: format!(
                                "{} - {} = {diff}, the claim asserts {asserted}",
                                claim.fields[1], claim.fields[0]
                            ),
                        },
                        None => DerivableVerdict::Unsupported {
                            field: "(asserted value is not a number)".to_string(),
                        },
                    }
                }
                (a, _b) => DerivableVerdict::Unsupported {
                    field: if a.is_none() {
                        claim.fields[0].clone()
                    } else {
                        claim.fields[1].clone()
                    },
                },
            }
        }
        ClaimKind::Count => {
            let Some(field) = claim.fields.first() else {
                return DerivableVerdict::Unsupported {
                    field: "(no field cited)".to_string(),
                };
            };
            match field_lookup(artifact, field) {
                Some(Value::Array(items)) => {
                    let n = items.len();
                    match claim.asserted.as_u64() {
                        Some(asserted) if asserted == n as u64 => DerivableVerdict::Supported,
                        Some(asserted) => DerivableVerdict::Contradicted {
                            reason: format!("{field} has {n} items, the claim asserts {asserted}"),
                        },
                        None => DerivableVerdict::Unsupported {
                            field: "(asserted count is not an integer)".to_string(),
                        },
                    }
                }
                Some(_) => DerivableVerdict::Unsupported {
                    field: field.clone(),
                },
                None => DerivableVerdict::Unsupported {
                    field: field.clone(),
                },
            }
        }
    }
}

/// Check EVERY claim against the artifact: the aggregate verdict is the
/// WORST one (any contradiction or unsupported claim blocks promotion -
/// a partially-supported claim set is not support).
pub fn check_all(claims: &[DerivableClaim], artifact: &Value) -> DerivableVerdict {
    let mut worst = DerivableVerdict::Supported;
    for claim in claims {
        let verdict = check(claim, artifact);
        match (&verdict, &worst) {
            (DerivableVerdict::Contradicted { .. }, _) => return verdict,
            (DerivableVerdict::Unsupported { .. }, DerivableVerdict::Supported) => {
                worst = verdict;
            }
            _ => {}
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    fn artifact() -> Value {
        json!({
            "run": {"tasks": 120, "failures": 3, "trials": [1, 2, 3, 4]},
            "tokens": {"prompt": 900, "completion": 100}
        })
    }

    #[test]
    fn derivable_claim_is_recomputed_not_judged() {
        // The cited total is RECOMPUTED from the artifact fields - the
        // engine does not grade prose, it does arithmetic.
        let claim = DerivableClaim {
            kind: ClaimKind::Sum,
            fields: vec!["tokens.prompt".into(), "tokens.completion".into()],
            asserted: json!(1000),
        };
        assert_eq!(check(&claim, &artifact()), DerivableVerdict::Supported);
        let wrong = DerivableClaim {
            asserted: json!(999),
            ..claim
        };
        assert!(matches!(
            check(&wrong, &artifact()),
            DerivableVerdict::Contradicted { .. }
        ));
    }

    #[test]
    fn value_must_trace_to_artifact_field() {
        let claim = DerivableClaim {
            kind: ClaimKind::FieldReference,
            fields: vec!["run.missing_field".into()],
            asserted: json!(42),
        };
        assert!(matches!(
            check(&claim, &artifact()),
            DerivableVerdict::Unsupported { .. }
        ));
    }

    #[test]
    fn contradicted_claim_blocks_promotion() {
        // A contradiction is a REFUSAL, not a downgrade - the enforcement
        // rule is part of the verdict type, not a caller's judgement call.
        let claim = DerivableClaim {
            kind: ClaimKind::FieldReference,
            fields: vec!["run.failures".into()],
            asserted: json!(0),
        };
        let verdict = check(&claim, &artifact());
        assert!(verdict.blocks_promotion());
        assert!(!DerivableVerdict::Supported.blocks_promotion());
    }

    #[test]
    fn count_is_recomputed_from_the_array_itself() {
        let claim = DerivableClaim {
            kind: ClaimKind::Count,
            fields: vec!["run.trials".into()],
            asserted: json!(4),
        };
        assert_eq!(check(&claim, &artifact()), DerivableVerdict::Supported);
        let overcount = DerivableClaim {
            asserted: json!(5),
            ..claim
        };
        assert!(matches!(
            check(&overcount, &artifact()),
            DerivableVerdict::Contradicted { .. }
        ));
    }

    #[test]
    fn difference_recomputes_both_operands() {
        let claim = DerivableClaim {
            kind: ClaimKind::Difference,
            fields: vec!["run.failures".into(), "run.tasks".into()],
            asserted: json!(117),
        };
        assert_eq!(check(&claim, &artifact()), DerivableVerdict::Supported);
    }

    #[test]
    fn any_unsupported_claim_blocks_the_whole_set() {
        // A partially-supported claim set is not support: the aggregate is
        // the WORST verdict.
        let good = DerivableClaim {
            kind: ClaimKind::FieldReference,
            fields: vec!["run.tasks".into()],
            asserted: json!(120),
        };
        let untraceable = DerivableClaim {
            kind: ClaimKind::FieldReference,
            fields: vec!["run.ghost".into()],
            asserted: json!(1),
        };
        let aggregate = check_all(&[good, untraceable], &artifact());
        assert!(aggregate.blocks_promotion());
    }
}
