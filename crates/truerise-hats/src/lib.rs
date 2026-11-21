//! truerise-hats: hat schema + TOML loader (spec §6 hats row - hats are
//! DATA, not traits). The loader validates the schema, computes the
//! content_hash, and aligns it into the audit chain (hash of the hat's
//! canonical form lands on the chain as a hat-registered event).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Loader errors.
#[derive(Debug, thiserror::Error)]
pub enum HatError {
    /// Malformed TOML or schema violation.
    #[error("hat: malformed: {0}")]
    Malformed(String),
    /// Unsupported schema version.
    #[error("hat: unsupported schema version {0}")]
    UnsupportedSchema(String),
    /// Chain (audit alignment) error.
    #[error("hat: chain: {0}")]
    Chain(String),
    /// G29: the skill declares a precondition that is a MARKED, UNWASHED
    /// unverified assumption - skills may not stand on unverified ground.
    #[error("hat: unverified precondition {text:?} - wash the assumption or drop the precondition")]
    UnverifiedPrecondition {
        /// The offending precondition text.
        text: String,
    },
}

/// The hat schema (v1): name / system_prompt / tool_subset / model_route /
/// discipline / schema_version / content_hash (computed by the loader).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hat {
    /// Schema version (must be "truerise.hat.v1").
    pub schema_version: String,
    /// Hat name (e.g. "researcher").
    pub name: String,
    /// The system prompt for this hat.
    pub system_prompt: String,
    /// The subset of tools this hat may use (deny-by-default outside subset).
    pub tool_subset: Vec<String>,
    /// Model route (which model this hat pins to).
    pub model_route: String,
    /// Discipline hints (free-form lines).
    #[serde(default)]
    pub discipline: Vec<String>,
    /// G29: assertions this skill assumes true. Each one must NOT match a
    /// marked, unwashed unverified assumption at load time. Typed so the
    /// preconditions participate in the content hash (a typo'd key can no
    /// longer silently skip the gate - review B3-P2-7).
    #[serde(default)]
    pub preconditions: Vec<String>,
}

impl Hat {
    /// The canonical form for hashing (JSON serialization of the struct).
    pub fn canonical(&self) -> String {
        serde_json::to_string(&self).unwrap_or_default()
    }

    /// The content hash (64 lowercase hex).
    pub fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical());
        let out = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for b in out.iter() {
            hex.push_str(&format!("{b:02x}"));
        }
        hex
    }
}

/// Parse + validate a hat WITHOUT touching the chain (the boundary may
/// validate further - e.g. the tool registry - BEFORE registering).
///
/// # Errors
/// [`HatError::Malformed`] / [`HatError::UnsupportedSchema`] on bad input.
pub fn parse_hat(toml_text: &str) -> Result<Hat, HatError> {
    let hat: Hat = toml::from_str(toml_text).map_err(|e| HatError::Malformed(e.to_string()))?;
    if hat.schema_version != "truerise.hat.v1" {
        return Err(HatError::UnsupportedSchema(hat.schema_version));
    }
    if hat.name.trim().is_empty() || hat.system_prompt.trim().is_empty() {
        return Err(HatError::Malformed(
            "name and system_prompt are required".into(),
        ));
    }
    Ok(hat)
}

/// The typed preconditions of a parsed hat (no raw TOML scan: the field
/// participates in the content hash, review B3-P2-7).
fn declared_preconditions(hat: &Hat) -> Vec<String> {
    hat.preconditions.clone()
}

/// G29: load a hat with the persistent-state write gate applied to its
/// preconditions. A skill declaring a precondition that is a MARKED,
/// UNWASHED unverified assumption is REJECTED - skills may not stand on
/// unverified ground (acceptance: `skill_load_rejects_unverified_precondition`).
///
/// # Errors
/// As [`load_hat`], plus [`HatError::UnverifiedPrecondition`].
pub fn load_hat_gated(
    toml_text: &str,
    chain: &truerise_governance::HashChain,
    ledger: &truerise_governance::UnverifiedLedger,
) -> Result<Hat, HatError> {
    // Parse FIRST (a malformed hat is a malformed hat, regardless of
    // preconditions), then match each declared precondition against the
    // unwashed marks - WHOLE-CONTENT digests AND per-line digests (review
    // A3-P2-3: whole-text matching alone made this gate degenerate, since
    // marks are keyed on entire proposals), then delegate to the plain
    // loader so the chain alignment stays identical.
    let hat = parse_hat(toml_text)?;
    for text in declared_preconditions(&hat) {
        let mut hasher = sha2::Sha256::new();
        hasher.update(text.as_bytes());
        let digest: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let marked_unwashed = ledger.entries().iter().any(|e| {
            e.path == truerise_governance::WritePath::ProposalApply
                && e.washed.is_none()
                && (e.assertion_digest == digest || e.line_digests.contains(&digest))
        });
        if marked_unwashed {
            return Err(HatError::UnverifiedPrecondition { text });
        }
    }
    load_hat(toml_text, chain)
}

/// Load a hat from raw TOML + align its content hash into the audit chain.
///
/// # Errors
/// [`HatError::Malformed`] / [`HatError::UnsupportedSchema`] on bad input;
/// [`HatError::Chain`] when the chain refuses the alignment event.
pub fn load_hat(toml_text: &str, chain: &truerise_governance::HashChain) -> Result<Hat, HatError> {
    let hat = parse_hat(toml_text)?;
    chain
        .append(
            "hat.registered",
            truerise_wire::Severity::Info,
            &serde_json::json!({
                "name": hat.name,
                "content_hash": hat.content_hash(),
                "model_route": hat.model_route,
            }),
            None,
        )
        .map_err(|e| HatError::Chain(e.to_string()))?;
    Ok(hat)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDir(PathBuf);
    impl Default for TempDir {
        fn default() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "truerise_hats_{}_{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn tmp_chain() -> (TempDir, truerise_governance::HashChain) {
        let dir = TempDir::default();
        let chain = truerise_governance::HashChain::open(&dir.0.join("hats.jsonl")).unwrap();
        (dir, chain)
    }

    const RESEARCHER: &str = r#"
schema_version = "truerise.hat.v1"
name = "researcher"
system_prompt = "You investigate hypotheses with falsifiable checks."
tool_subset = ["read_file", "repo_grep", "list_dir"]
model_route = "default"
discipline = ["no unverifiable claims", "cite rows"]
"#;

    /// G29 acceptance: a skill declaring a precondition that is a marked,
    /// unwashed unverified assumption is REJECTED; washing the assumption
    /// (with a legitimate support kind) unblocks the load.
    #[test]
    fn gated_skill_load_rejects_unverified_precondition() {
        let dir = tempfile::tempdir().unwrap();
        let chain = truerise_governance::HashChain::open(&dir.path().join("c.jsonl")).unwrap();
        let mut ledger = truerise_governance::UnverifiedLedger::new();
        let gate = truerise_governance::WriteGate::new(0);
        let assertion = "the patched harness passes the full bench";
        // The assumption enters persistent state via the proposal-apply
        // path without support: marked, not washed.
        gate.decide(
            truerise_governance::WritePath::ProposalApply,
            assertion,
            None,
            &mut ledger,
            Some(&chain),
        )
        .unwrap();
        let hat_toml = format!(
            r#"schema_version = "truerise.hat.v1"
name = "patched-evaluator"
system_prompt = "judge the run"
tool_subset = ["read"]
model_route = "main"
preconditions = ["{assertion}"]
"#
        );
        assert!(matches!(
            load_hat_gated(&hat_toml, &chain, &ledger),
            Err(HatError::UnverifiedPrecondition { text })
            if text == assertion
        ));
        // A hat with no preconditions loads regardless.
        let plain = r#"schema_version = "truerise.hat.v1"
name = "plain"
system_prompt = "work"
tool_subset = ["read"]
model_route = "main"
"#;
        assert!(load_hat_gated(plain, &chain, &ledger).is_ok());
        // Washing with a legitimate support kind unblocks the load - and
        // the wash action itself lands on the chain.
        let id = ledger.entries()[0].id.clone();
        ledger
            .wash(
                &id,
                &truerise_governance::SupportRef::Recomputation {
                    claim: "gen1 replay".to_string(),
                    digest: "abc".to_string(),
                },
                1,
                Some(&chain),
            )
            .unwrap();
        assert!(load_hat_gated(&hat_toml, &chain, &ledger).is_ok());
    }

    #[test]
    fn toml_hat_loads_and_aligns_to_chain() {
        let (_dir, chain) = tmp_chain();
        let hat = load_hat(RESEARCHER, &chain).unwrap();
        assert_eq!(hat.name, "researcher");
        assert_eq!(hat.tool_subset.len(), 3);
        assert_eq!(hat.content_hash().len(), 64);
        // Audit alignment: the chain carries the hat-registered event.
        let rows = chain.governing_rows();
        assert!(rows.iter().any(|r| r.event.kind == "hat.registered"
            && r.event.detail.get("content_hash") == Some(&serde_json::json!(hat.content_hash()))));
    }

    #[test]
    fn wrong_schema_version_refused() {
        let (_dir, chain) = tmp_chain();
        let bad = RESEARCHER.replace("truerise.hat.v1", "other.v9");
        assert!(matches!(
            load_hat(&bad, &chain),
            Err(HatError::UnsupportedSchema(_))
        ));
    }

    #[test]
    fn content_hash_binds_content() {
        let (_dir, chain) = tmp_chain();
        let hat = load_hat(RESEARCHER, &chain).unwrap();
        let changed: String = RESEARCHER.replace("falsifiable checks", "falsifiable CHECKS");
        let (_d2, chain2) = tmp_chain();
        let hat2 = load_hat(&changed, &chain2).unwrap();
        assert_ne!(
            hat.content_hash(),
            hat2.content_hash(),
            "any content change -> new hash"
        );
    }
}
