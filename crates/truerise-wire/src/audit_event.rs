//! Audit event wire contracts - two explicit editions.
//!
//! - [`AuditEventV1`]: the legacy edition. Loose (extra fields allowed),
//!   still deserializable for backward compatibility, but deprecated and
//!   excluded from governance (read-only compatibility).
//! - [`AuditEventV2`]: the tightened edition - no extra fields (serde
//!   `deny_unknown_fields`), the three chain-critical keys are mandatory
//!   struct fields, hashes are format-checked on the wire.

use serde::{Deserialize, Serialize};

/// Event severity (shared by both editions; spec enum, 4 values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Debug-level event.
    Debug,
    /// Informational event.
    Info,
    /// Warning event.
    Warn,
    /// Error event.
    Error,
}

/// SHA-256 hex digest as it appears on the wire (64 lowercase hex chars).
fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The legacy edition (v1). Deprecated: kept for wire compatibility only.
///
/// Extra fields are preserved verbatim (loose schema). Use
/// [`AuditEventV1::governs`] - it is always `false` - as the governance
/// exclusion marker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEventV1 {
    /// Monotonic sequence number within one chain.
    pub seq: u64,
    /// Event timestamp (UTC ISO-8601, free-form on v1).
    pub ts: String,
    /// Event kind discriminator.
    pub kind: String,
    /// Event severity.
    pub severity: Severity,
    /// Structured payload (loose).
    pub detail: serde_json::Value,
    /// Previous row hash (legacy rows may carry unpinned forms).
    pub prev_hash: String,
    /// Row hash (legacy rows may carry unpinned forms).
    pub hash: String,
    /// Extra fields preserved verbatim (loose edition).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl AuditEventV1 {
    /// Governance participation: legacy rows NEVER participate (deprecated).
    pub fn governs(&self) -> bool {
        false
    }
}

/// The tightened edition (v2): no extra fields, chain-critical keys required,
/// hashes format-checked. This is the governing edition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditEventV2 {
    /// Monotonic sequence number within one chain.
    pub seq: u64,
    /// Event timestamp, UTC ISO-8601.
    pub ts: String,
    /// Event kind discriminator (non-empty on the wire).
    pub kind: String,
    /// Event severity (4-value enum).
    pub severity: Severity,
    /// Structured payload.
    pub detail: serde_json::Value,
    /// Previous row hash: 64 lowercase hex (genesis = 64 zeros).
    #[serde(deserialize_with = "de_sha256")]
    pub prev_hash: String,
    /// Row hash over the canonical serialization of all fields except hash.
    #[serde(deserialize_with = "de_sha256")]
    pub hash: String,
}

impl AuditEventV2 {
    /// Governance participation: the tightened edition always governs.
    pub fn governs(&self) -> bool {
        true
    }
}

fn de_sha256<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if is_sha256_hex(&s) {
        Ok(s)
    } else {
        Err(serde::de::Error::custom(
            "hash must be 64 lowercase hex chars",
        ))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn v2_json() -> serde_json::Value {
        serde_json::json!({
            "seq": 0,
            "ts": "2026-11-14T00:00:00Z",
            "kind": "chain.genesis",
            "severity": "info",
            "detail": {},
            "prev_hash": "0".repeat(64),
            "hash": "a".repeat(64)
        })
    }

    #[test]
    fn v2_roundtrip() {
        let ev: AuditEventV2 = serde_json::from_value(v2_json()).expect("valid v2");
        assert!(ev.governs());
        let back = serde_json::to_value(&ev).unwrap();
        assert_eq!(back, v2_json(), "v2 serde round-trip must be exact");
    }

    #[test]
    fn v2_rejects_unknown_fields() {
        let mut bad = v2_json();
        bad["surprise"] = serde_json::json!(1);
        let res: Result<AuditEventV2, _> = serde_json::from_value(bad);
        assert!(res.is_err(), "tightened edition forbids extra fields");
    }

    #[test]
    fn v2_rejects_bad_hash_format() {
        let mut bad = v2_json();
        bad["hash"] = serde_json::json!("not-a-hash");
        let res: Result<AuditEventV2, _> = serde_json::from_value(bad);
        assert!(res.is_err(), "hash format is wire-checked");
    }

    #[test]
    fn v1_preserves_extra_fields_and_never_governs() {
        let v1_json = serde_json::json!({
            "seq": 0, "ts": "t", "kind": "k", "severity": "info",
            "detail": {}, "prev_hash": "p", "hash": "h",
            "legacy_note": "kept verbatim"
        });
        let ev: AuditEventV1 = serde_json::from_value(v1_json).expect("v1 loose");
        assert!(
            !ev.governs(),
            "legacy edition never participates in governance"
        );
        assert_eq!(
            ev.extra.get("legacy_note").and_then(|v| v.as_str()),
            Some("kept verbatim")
        );
        let back = serde_json::to_value(&ev).unwrap();
        assert_eq!(back["legacy_note"], serde_json::json!("kept verbatim"));
    }

    #[test]
    fn schema_file_alignment() {
        // The shipped JSON schema (schemas/audit_event.v2.schema.json) must
        // describe exactly the serde field set of AuditEventV2 - spec-level
        // alignment (fields), not file references.
        let schema_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/audit_event.v2.schema.json"
        );
        let schema: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(schema_path).expect("schema file present"),
        )
        .unwrap();
        let props = schema["properties"].as_object().expect("properties object");
        let wire_fields = [
            "seq",
            "ts",
            "kind",
            "severity",
            "detail",
            "prev_hash",
            "hash",
        ];
        for f in wire_fields {
            assert!(props.contains_key(f), "schema missing wire field {f}");
        }
        assert_eq!(
            props.len(),
            wire_fields.len(),
            "schema has exactly the wire fields"
        );
        assert_eq!(schema["additionalProperties"], serde_json::json!(false));
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 3, "three chain-critical required keys");
    }
}
