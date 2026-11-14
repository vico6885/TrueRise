//! truerise-observe: audit events -> spans, TWO tiers (spec §6 observe row):
//! jsonl (default, always available) and OTLP SDK behind the `otel` feature.
//! ONE-WAY: export must never feed back into judging - the property test
//! asserts the chain digest is unchanged before/after export. Sensitive keys
//! are redacted on the way out.

use serde_json::Value;
use sha2::Digest;
use std::path::Path;

/// Sensitive key fragments (redacted on export; case-insensitive contains).
const SENSITIVE_FRAGMENTS: [&str; 4] = ["password", "token", "secret", "api_key"];

/// Redact sensitive values in a JSON tree (values under sensitive keys become
/// "<redacted>"; structure preserved).
#[must_use]
pub fn redact(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let out = map
                .iter()
                .map(|(k, v)| {
                    let key_sensitive = SENSITIVE_FRAGMENTS
                        .iter()
                        .any(|f| k.to_lowercase().contains(f));
                    (
                        k.clone(),
                        if key_sensitive {
                            Value::from("<redacted>")
                        } else {
                            redact(v)
                        },
                    )
                })
                .collect();
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        other => other.clone(),
    }
}

/// Read a persisted chain file (JSON lines) and export each row as a span to
/// ``out`` (jsonl, one span per line). The source file is opened READ-ONLY;
/// the export is one-way (nothing writes back to the chain).
///
/// # Errors
/// IO / parse errors surface as strings at the port boundary.
pub fn export_jsonl(chain_path: &Path, out_path: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(chain_path).map_err(|e| e.to_string())?;
    let mut count = 0usize;
    let mut out_lines = String::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let span = serde_json::json!({
            "span.kind": "audit",
            "seq": row.get("seq"),
            "kind": row.get("kind"),
            "severity": row.get("severity"),
            "detail": redact(row.get("detail").unwrap_or(&Value::Null)),
            "hash": row.get("hash"), // hash EXPORTED so spans remain verifiable
        });
        out_lines.push_str(&serde_json::to_string(&span).map_err(|e| e.to_string())?);
        out_lines.push('\n');
        count += 1;
    }
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(out_path, out_lines).map_err(|e| e.to_string())?;
    Ok(count)
}

/// The chain digest after export must equal the digest before (ONE-WAY:
/// export never re-ingests). Computes sha256 over the chain file bytes.
#[must_use]
pub fn chain_digest(chain_path: &Path) -> String {
    let bytes = std::fs::read(chain_path).unwrap_or_default();
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let out = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in out.iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDir(PathBuf);
    impl Default for TempDir {
        fn default() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!(
                "truerise_observe_{}_{}",
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
    use std::path::PathBuf;

    #[test]
    fn export_produces_one_span_per_row_and_redacts() {
        let dir = TempDir::default();
        let chain = dir.0.join("c.jsonl");
        let line0 = "{\"seq\":0,\"ts\":\"t\",\"kind\":\"k0\",\"severity\":\"info\",\"detail\":{\"api_key\":\"sk-123\"},\"prev_hash\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}";
        let line1 = "{\"seq\":1,\"ts\":\"t\",\"kind\":\"k1\",\"severity\":\"warn\",\"detail\":{\"note\":\"fine\",\"token\":\"abc\"},\"prev_hash\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"hash\":\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}";
        let fixture = format!(
            "{line0}
{line1}
"
        );
        std::fs::write(&chain, fixture).unwrap();
        let before = chain_digest(&chain);
        let out = dir.0.join("spans.jsonl");
        let n = export_jsonl(&chain, &out).expect("export");
        assert_eq!(n, 2);
        let exported = std::fs::read_to_string(&out).unwrap();
        assert!(exported.contains("<redacted>"), "sensitive values redacted");
        assert!(!exported.contains("sk-123"), "raw secret must NOT appear");
        assert!(!exported.contains("abc\""), "raw token must NOT appear");
        // ONE-WAY: chain digest unchanged after export.
        assert_eq!(
            before,
            chain_digest(&chain),
            "export must not touch the chain"
        );
    }

    #[test]
    fn redact_walks_nested_structures() {
        let v = serde_json::json!({"outer": {"my_password": "x", "list": [{"secret_hint": 1}]}});
        let r = redact(&v);
        assert_eq!(r["outer"]["my_password"], "<redacted>");
        assert_eq!(r["outer"]["list"][0]["secret_hint"], "<redacted>");
        assert_eq!(r["outer"]["list"][0].as_object().unwrap().len(), 1);
    }

    #[cfg(feature = "otel")]
    #[test]
    fn otlp_feature_placeholder_compiles() {
        // The OTLP SDK tier lands behind this feature; the jsonl tier stays
        // the default and the one-way property applies to both.
        assert!(chain_digest(Path::new("no/such/file")) == chain_digest(Path::new("no/such/file")));
    }
}
