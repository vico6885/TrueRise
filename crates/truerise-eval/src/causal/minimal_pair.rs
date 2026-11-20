//! Minimal pairs (CA4): the input-side intervention primitive.
//!
//! A minimal pair is two task-pack variants that differ in EXACTLY ONE
//! declared element - swap the element, rerun, and the paired outcome
//! delta prices that element's effect with the sign-flip machinery
//! ([`super::permutation::paired_flip`]). This is the honest agent-side
//! replacement for observational mediation: you do not ASSUME the
//! transmission chain, you cut one wire and measure.
//!
//! Boundary: a minimal pair prices an INPUT ELEMENT's effect, not a
//! component's - component credit belongs to the interventional Shapley
//! family ([`super::shapley`]).

/// Pair-validation refusals.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairError {
    /// The declared field must be the ONLY difference; anything else is a
    /// confounded pair and the delta would price a bundle, not an element.
    #[error(
        "minimal pair refused: baseline and variant differ in {differences} elements ({fields}) - declared field is {declared:?}"
    )]
    NotMinimal {
        /// How many elements differ.
        differences: usize,
        /// The differing element names (sorted).
        fields: String,
        /// The field the caller declared.
        declared: String,
    },
    /// The declared field does not exist in both variants.
    #[error("minimal pair refused: declared field {0:?} missing from a variant")]
    DeclaredFieldMissing(String),
    /// The declared field holds the SAME value on both sides - there is no
    /// intervention at all.
    #[error("minimal pair refused: declared field {0:?} is identical on both sides")]
    NoIntervention(String),
    /// Element removal must be encoded as an empty value, not a missing
    /// key: both variants must carry the same key set.
    #[error(
        "minimal pair refused: key set differs between variants ({missing_baseline:?} missing from baseline, {missing_variant:?} missing from variant)"
    )]
    KeySetsDiffer {
        /// Keys only in the variant (missing from baseline).
        missing_baseline: Vec<String>,
        /// Keys only in the baseline (missing from variant).
        missing_variant: Vec<String>,
    },
    /// A key appears twice on one side - malformed input, not a key-set
    /// difference (review A2-P2-5: the old shape overloaded KeySetsDiffer).
    #[error("minimal pair refused: duplicate key {key:?} in the {side}")]
    DuplicateKey {
        /// Which side carries the duplicate.
        side: &'static str,
        /// The duplicated key.
        key: String,
    },
}

/// Validate a minimal pair: same key set on both sides, exactly ONE
/// differing element, and it must be the declared field.
///
/// Element maps are ordered key-value lists; duplicate keys are refused as
/// malformed input (the last-wins semantics of maps would hide a real
/// difference).
pub fn validate_minimal_pair(
    baseline: &[(String, String)],
    variant: &[(String, String)],
    declared: &str,
) -> Result<(), PairError> {
    for (side_name, map) in [("baseline", baseline), ("variant", variant)] {
        let mut keys: Vec<&String> = map.iter().map(|(k, _)| k).collect();
        keys.sort();
        for pair in keys.windows(2) {
            if pair[0] == pair[1] {
                return Err(PairError::DuplicateKey {
                    side: side_name,
                    key: pair[0].clone(),
                });
            }
        }
    }
    let baseline_keys: std::collections::BTreeSet<&String> =
        baseline.iter().map(|(k, _)| k).collect();
    let variant_keys: std::collections::BTreeSet<&String> =
        variant.iter().map(|(k, _)| k).collect();
    let only_variant: Vec<String> = variant_keys
        .difference(&baseline_keys)
        .map(|s| s.to_string())
        .collect();
    let only_baseline: Vec<String> = baseline_keys
        .difference(&variant_keys)
        .map(|s| s.to_string())
        .collect();
    if !only_variant.is_empty() || !only_baseline.is_empty() {
        return Err(PairError::KeySetsDiffer {
            missing_baseline: only_variant,
            missing_variant: only_baseline,
        });
    }
    let value = |map: &[(String, String)], key: &str| -> Option<String> {
        map.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    };
    let Some(baseline_value) = value(baseline, declared) else {
        return Err(PairError::DeclaredFieldMissing(declared.to_string()));
    };
    let Some(variant_value) = value(variant, declared) else {
        return Err(PairError::DeclaredFieldMissing(declared.to_string()));
    };
    if baseline_value == variant_value {
        return Err(PairError::NoIntervention(declared.to_string()));
    }
    let differences: Vec<String> = baseline_keys
        .iter()
        .filter_map(|k| {
            let b = value(baseline, k);
            let v = value(variant, k);
            if b == v { None } else { Some(k.to_string()) }
        })
        .collect();
    if differences.len() != 1 || differences[0] != declared {
        return Err(PairError::NotMinimal {
            differences: differences.len(),
            fields: differences.join(", "),
            declared: declared.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn a_true_minimal_pair_validates() {
        let baseline = map(&[("hint", "on"), ("budget", "50"), ("domain", "math")]);
        let variant = map(&[("hint", "off"), ("budget", "50"), ("domain", "math")]);
        assert!(validate_minimal_pair(&baseline, &variant, "hint").is_ok());
    }

    #[test]
    fn a_two_element_difference_is_confounded_and_refused() {
        let baseline = map(&[("hint", "on"), ("budget", "50")]);
        let variant = map(&[("hint", "off"), ("budget", "80")]);
        let err = validate_minimal_pair(&baseline, &variant, "hint").unwrap_err();
        assert!(matches!(err, PairError::NotMinimal { differences: 2, .. }));
    }

    #[test]
    fn declaring_the_wrong_field_is_refused() {
        let baseline = map(&[("hint", "on"), ("budget", "50")]);
        let variant = map(&[("hint", "off"), ("budget", "50")]);
        // Declaring "budget", which is identical: no intervention at all.
        assert!(matches!(
            validate_minimal_pair(&baseline, &variant, "budget"),
            Err(PairError::NoIntervention(field))
            if field == "budget"
        ));
        // Declaring a field that does not exist.
        assert!(matches!(
            validate_minimal_pair(&baseline, &variant, "domain"),
            Err(PairError::DeclaredFieldMissing(_))
        ));
    }

    #[test]
    fn missing_keys_are_key_set_violations_not_differences() {
        let baseline = map(&[("hint", "on"), ("budget", "50")]);
        let variant = map(&[("hint", "off")]);
        let err = validate_minimal_pair(&baseline, &variant, "hint").unwrap_err();
        assert!(
            matches!(err, PairError::KeySetsDiffer { ref missing_variant, .. }
            if missing_variant.iter().any(|s| s.contains("budget")))
        );
        // Duplicate keys inside one side: malformed, refused by NAME.
        let dupe = vec![
            ("hint".to_string(), "on".to_string()),
            ("hint".to_string(), "off".to_string()),
        ];
        assert!(matches!(
            validate_minimal_pair(&dupe, &variant, "hint"),
            Err(PairError::DuplicateKey {
                side: "baseline",
                key
            })
            if key == "hint"
        ));
    }
}
