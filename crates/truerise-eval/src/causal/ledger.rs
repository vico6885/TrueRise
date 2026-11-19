//! The assumption ledger (CA2): the four identification assumptions as
//! first-class, machine-checkable rows.
//!
//! "No assumption row, no causal claim" - the causal mirror of the
//! spec-mapping gate. A causal claim in this repository must carry a row
//! that declares its identification assumptions (SUTVA, consistency,
//! exchangeability, positivity), names the defense and the evidence
//! pointers. The row is STRUCTURE; whether the defense convinces is a
//! judgement, and the tier semantics in [`super::typology`] encode the
//! honest fallback: an incomplete or absent row DOWNGRADES the claim to
//! hypothesis tier, it never upgrades it.

/// The four identification assumptions every causal claim must declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assumption {
    /// SUTVA: no interference between units, no hidden treatment versions.
    Sutva,
    /// Consistency: the observed outcome under treatment t IS the
    /// potential outcome Y(t).
    Consistency,
    /// Exchangeability (unconfoundedness): treatment assignment is
    /// independent of the potential outcomes given the observed covariates.
    Exchangeability,
    /// Positivity (overlap): every covariate stratum has a non-zero chance
    /// of each treatment arm.
    Positivity,
}

/// Parse/validate refusals for the ledger.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LedgerError {
    /// The registry text is not valid JSON.
    #[error("ledger refused: registry is not valid JSON: {0}")]
    Unparsable(String),
    /// A row's claim term is empty - it names nothing to defend.
    #[error("ledger refused: row {index} carries an empty claim_term")]
    EmptyClaimTerm {
        /// The row position.
        index: usize,
    },
    /// Two rows defend the same claim term - which one would the gate read?
    #[error("ledger refused: duplicate rows for claim term {term:?}")]
    DuplicateTerm {
        /// The duplicated term.
        term: String,
    },
    /// An assumption token outside the four-name vocabulary.
    #[error("ledger refused: row {index}: unknown assumption token {token:?}")]
    UnknownAssumption {
        /// The row position.
        index: usize,
        /// The offending token.
        token: String,
    },
    /// A status token outside the two-value vocabulary.
    #[error("ledger refused: row {index}: unknown status token {status:?}")]
    UnknownStatus {
        /// The row position.
        index: usize,
        /// The offending token.
        status: String,
    },
    /// An empty assumption list: a defense of nothing is a defense of
    /// nothing.
    #[error("ledger refused: row {index} declares no assumptions")]
    NoAssumptions {
        /// The row position.
        index: usize,
    },
    /// Evidence pointers must be stated even while undefended (the claim
    /// gate's own rule, mirrored here).
    #[error(
        "ledger refused: row {index} leaves field {field} empty - the promise must be stated even while undefended"
    )]
    EmptyEvidenceField {
        /// The row position.
        index: usize,
        /// The offending field name.
        field: &'static str,
    },
    /// The same assumption declared twice in one row.
    #[error("ledger refused: row {index} declares {assumption:?} twice")]
    DuplicateAssumption {
        /// The row position.
        index: usize,
        /// The duplicated assumption.
        assumption: Assumption,
    },
}

impl Assumption {
    /// Parse the snake_case registry token.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "sutva" => Some(Self::Sutva),
            "consistency" => Some(Self::Consistency),
            "exchangeability" => Some(Self::Exchangeability),
            "positivity" => Some(Self::Positivity),
            _ => None,
        }
    }

    /// The canonical token.
    #[must_use]
    pub fn token(&self) -> &'static str {
        match self {
            Self::Sutva => "sutva",
            Self::Consistency => "consistency",
            Self::Exchangeability => "exchangeability",
            Self::Positivity => "positivity",
        }
    }
}

/// The row's defense status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowStatus {
    /// The defense is asserted (its quality is judged elsewhere; the tier
    /// semantics only require COMPLETENESS, see [`AssumptionRow::is_complete`]).
    Defended,
    /// The defense is explicitly not yet made - the claim must be worded
    /// as a hypothesis.
    Undefended,
}

impl RowStatus {
    /// Parse the registry token.
    #[must_use]
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "defended" => Some(Self::Defended),
            "undefended" => Some(Self::Undefended),
            _ => None,
        }
    }
}

/// One claim's assumption row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumptionRow {
    /// The claim term this row defends (matches a `claim_terms.json` term).
    pub claim_term: String,
    /// The declared identification assumptions.
    pub assumptions: Vec<Assumption>,
    /// The defense text: WHY each declared assumption is defensible here.
    pub defense: String,
    /// The named test that exercises the sensitivity/refuter evidence.
    pub test: String,
    /// The committed artifact carrying the defense's working.
    pub artifact: String,
    /// The defense status.
    pub status: RowStatus,
}

impl AssumptionRow {
    /// The four assumptions, as a fixed-order reference list.
    #[must_use]
    pub fn all_assumptions() -> [Assumption; 4] {
        [
            Assumption::Sutva,
            Assumption::Consistency,
            Assumption::Exchangeability,
            Assumption::Positivity,
        ]
    }

    /// The declared assumptions that are MISSING from this row.
    #[must_use]
    pub fn missing_assumptions(&self) -> Vec<Assumption> {
        Self::all_assumptions()
            .into_iter()
            .filter(|a| !self.assumptions.contains(a))
            .collect()
    }

    /// Completeness: defended status AND all four assumptions declared AND
    /// the defense text present. Anything less downgrades the claim to
    /// hypothesis tier (see [`super::typology::tier_for`]).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.status == RowStatus::Defended
            && self.missing_assumptions().is_empty()
            && !self.defense.trim().is_empty()
    }
}

/// The registry: one row per causal claim term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumptionLedger {
    rows: Vec<AssumptionRow>,
}

impl AssumptionLedger {
    /// Parse the registry from its JSON text, enforcing structural rules:
    /// known tokens, non-empty evidence pointers, no duplicates.
    ///
    /// # Errors
    /// [`LedgerError`] - every variant names the offending row.
    pub fn from_json(text: &str) -> Result<Self, LedgerError> {
        let doc: serde_json::Value =
            serde_json::from_str(text).map_err(|e| LedgerError::Unparsable(e.to_string()))?;
        let raw_rows = doc
            .get("rows")
            .and_then(|r| r.as_array())
            .ok_or_else(|| LedgerError::Unparsable("missing 'rows' array".to_string()))?;
        let mut rows = Vec::with_capacity(raw_rows.len());
        for (index, raw) in raw_rows.iter().enumerate() {
            let term = raw
                .get("claim_term")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if term.trim().is_empty() {
                return Err(LedgerError::EmptyClaimTerm { index });
            }
            let mut assumptions = Vec::new();
            for token_value in raw
                .get("assumptions")
                .and_then(|v| v.as_array())
                .map(|a| a.iter().collect::<Vec<_>>())
                .unwrap_or_default()
            {
                // Review P2-4: a non-string token (e.g. a number) must be
                // REFUSED, not silently dropped - the runtime may never be
                // more lenient than the gate.
                let Some(token) = token_value.as_str() else {
                    return Err(LedgerError::UnknownAssumption {
                        index,
                        token: token_value.to_string(),
                    });
                };
                let Some(assumption) = Assumption::parse(token) else {
                    return Err(LedgerError::UnknownAssumption {
                        index,
                        token: token.to_string(),
                    });
                };
                if assumptions.contains(&assumption) {
                    return Err(LedgerError::DuplicateAssumption { index, assumption });
                }
                assumptions.push(assumption);
            }
            if assumptions.is_empty() {
                return Err(LedgerError::NoAssumptions { index });
            }
            let status_token = raw.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let Some(status) = RowStatus::parse(status_token) else {
                return Err(LedgerError::UnknownStatus {
                    index,
                    status: status_token.to_string(),
                });
            };
            for (field, value) in [
                (
                    "defense",
                    raw.get("defense").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                (
                    "test",
                    raw.get("test").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                (
                    "artifact",
                    raw.get("artifact").and_then(|v| v.as_str()).unwrap_or(""),
                ),
            ] {
                if value.trim().is_empty() {
                    return Err(LedgerError::EmptyEvidenceField { index, field });
                }
            }
            rows.push(AssumptionRow {
                claim_term: term,
                assumptions,
                defense: raw
                    .get("defense")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                test: raw
                    .get("test")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                artifact: raw
                    .get("artifact")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                status,
            });
        }
        for (i, row) in rows.iter().enumerate() {
            if rows[i + 1..]
                .iter()
                .any(|other| other.claim_term == row.claim_term)
            {
                return Err(LedgerError::DuplicateTerm {
                    term: row.claim_term.clone(),
                });
            }
        }
        Ok(Self { rows })
    }

    /// The rows, in registry order.
    #[must_use]
    pub fn rows(&self) -> &[AssumptionRow] {
        &self.rows
    }

    /// The row defending a given claim term, if present.
    #[must_use]
    pub fn row_for(&self, term: &str) -> Option<&AssumptionRow> {
        self.rows.iter().find(|r| r.claim_term == term)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    const GOOD_ROW: &str = r#"{
        "claim_term": "Memory Recall Lifts Task Success",
        "assumptions": ["sutva", "consistency", "exchangeability", "positivity"],
        "defense": "runs are cluster-randomised per session; recall scores are logged on the chain",
        "test": "causal_interventional_without_defense_downgrades_to_hypothesis",
        "artifact": "readings/v0.2/gen1_certificate.json",
        "status": "defended"
    }"#;

    #[test]
    fn a_complete_defended_row_passes_structure_and_completeness() {
        let ledger = AssumptionLedger::from_json(&format!(r#"{{"rows": [{GOOD_ROW}]}}"#)).unwrap();
        let row = ledger.row_for("Memory Recall Lifts Task Success").unwrap();
        assert!(row.is_complete());
        assert!(row.missing_assumptions().is_empty());
    }

    #[test]
    fn a_defended_row_missing_one_assumption_is_structurally_valid_but_incomplete() {
        // Positivity is the assumption most often smuggled in - leaving it
        // undeclared must show up as incompleteness, not as an error.
        let text = GOOD_ROW.replace(r#""exchangeability", "positivity""#, r#""exchangeability""#);
        let ledger = AssumptionLedger::from_json(&format!(r#"{{"rows": [{text}]}}"#)).unwrap();
        let row = ledger.row_for("Memory Recall Lifts Task Success").unwrap();
        assert!(!row.is_complete());
        assert_eq!(row.missing_assumptions(), vec![Assumption::Positivity]);
    }

    #[test]
    fn undefended_rows_are_legal_but_incomplete() {
        let text = GOOD_ROW.replace(r#""defended""#, r#""undefended""#);
        let ledger = AssumptionLedger::from_json(&format!(r#"{{"rows": [{text}]}}"#)).unwrap();
        let row = ledger.row_for("Memory Recall Lifts Task Success").unwrap();
        assert!(!row.is_complete(), "undefended must never read as complete");
    }

    #[test]
    fn structural_violations_are_refused_by_name() {
        let wrap = |row: &str| format!(r#"{{"rows": [{row}]}}"#);
        // Unknown assumption token.
        let bad = GOOD_ROW.replace(r#""sutva", "#, r#""spyglass", "#);
        assert!(matches!(
            AssumptionLedger::from_json(&wrap(&bad)),
            Err(LedgerError::UnknownAssumption { index: 0, .. })
        ));
        // Review P2-4: a NON-STRING token must be refused, never dropped.
        let bad = GOOD_ROW.replace(r#""sutva", "#, r#"42, "#);
        assert!(matches!(
            AssumptionLedger::from_json(&wrap(&bad)),
            Err(LedgerError::UnknownAssumption { index: 0, token })
            if token == "42"
        ));
        // No assumptions at all.
        let bad = GOOD_ROW.replace(
            r#"["sutva", "consistency", "exchangeability", "positivity"]"#,
            "[]",
        );
        assert!(matches!(
            AssumptionLedger::from_json(&wrap(&bad)),
            Err(LedgerError::NoAssumptions { index: 0 })
        ));
        // Unknown status.
        let bad = GOOD_ROW.replace(r#""defended""#, r#""believed""#);
        assert!(matches!(
            AssumptionLedger::from_json(&wrap(&bad)),
            Err(LedgerError::UnknownStatus { index: 0, .. })
        ));
        // Empty evidence field.
        let bad = GOOD_ROW.replace(
            r#""artifact": "readings/v0.2/gen1_certificate.json""#,
            r#""artifact": """#,
        );
        assert!(matches!(
            AssumptionLedger::from_json(&wrap(&bad)),
            Err(LedgerError::EmptyEvidenceField {
                field: "artifact",
                ..
            })
        ));
        // Duplicate claim term.
        let ledger_text = format!(r#"{{"rows": [{GOOD_ROW}, {GOOD_ROW}]}}"#);
        assert!(matches!(
            AssumptionLedger::from_json(&ledger_text),
            Err(LedgerError::DuplicateTerm { .. })
        ));
        // Unparsable registry.
        assert!(matches!(
            AssumptionLedger::from_json("not json"),
            Err(LedgerError::Unparsable(_))
        ));
        // Empty claim term.
        let bad = GOOD_ROW.replace(
            r#""claim_term": "Memory Recall Lifts Task Success""#,
            r#""claim_term": """#,
        );
        assert!(matches!(
            AssumptionLedger::from_json(&wrap(&bad)),
            Err(LedgerError::EmptyClaimTerm { index: 0 })
        ));
    }

    #[test]
    fn an_empty_registry_is_legal_and_answers_none() {
        let ledger = AssumptionLedger::from_json(r#"{"rows": []}"#).unwrap();
        assert!(ledger.rows().is_empty());
        assert!(ledger.row_for("anything").is_none());
    }
}
