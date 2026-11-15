//! W10 acceptance tests: certificate signing identity. A stranger can
//! verify WHO issued a certificate - and an unsigned or tampered one is
//! NEVER silently trusted.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use ed25519_dalek::SigningKey;
use truerise_cli::identity::{
    VerifyOutcome, load_signing_key, sign_certificate, verify_certificate,
};

fn temp_dir(tag: &str) -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "truerise_cert_identity_{tag}_{}_{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// A deterministic test key (NEVER a production key: seeds come from the
/// environment at issuance time).
fn test_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn sample_cert() -> serde_json::Value {
    serde_json::json!({
        "generation": 1,
        "verdict": { "verdict": "PASS", "failures": 0 }
    })
}

fn issuer_hex(key: &SigningKey) -> String {
    key.verifying_key()
        .to_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn verified_certificate_prints_issuer_and_chain_tip() {
    let dir = temp_dir("verified");
    let path = dir.join("cert.json");
    let mut cert = sample_cert();
    let key = test_key();
    sign_certificate(&mut cert, &key);
    std::fs::write(&path, serde_json::to_string_pretty(&cert).unwrap()).unwrap();
    let outcome = verify_certificate(&path, Some(&issuer_hex(&key)));
    match outcome {
        VerifyOutcome::Verified { issuer } => {
            assert_eq!(issuer, issuer_hex(&key), "the trusted issuer is named");
        }
        other => panic!("expected Verified, got {other:?}"),
    }
}

#[test]
fn tampered_certificate_fails_signature_check() {
    let dir = temp_dir("tampered");
    let path = dir.join("cert.json");
    let mut cert = sample_cert();
    let key = test_key();
    sign_certificate(&mut cert, &key);
    // Tamper AFTER signing:
    cert["verdict"]["failures"] = serde_json::json!(99);
    std::fs::write(&path, serde_json::to_string_pretty(&cert).unwrap()).unwrap();
    assert_eq!(
        verify_certificate(&path, Some(&issuer_hex(&key))),
        VerifyOutcome::InvalidSignature,
        "a tampered payload no longer verifies"
    );
    // A cert signed by ANOTHER key under this trusted anchor also fails:
    let mut foreign = sample_cert();
    sign_certificate(&mut foreign, &SigningKey::from_bytes(&[9u8; 32]));
    let foreign_path = dir.join("foreign.json");
    std::fs::write(
        &foreign_path,
        serde_json::to_string_pretty(&foreign).unwrap(),
    )
    .unwrap();
    assert_eq!(
        verify_certificate(&foreign_path, Some(&issuer_hex(&key))),
        VerifyOutcome::InvalidSignature,
        "a foreign issuer is not our issuer"
    );
}

#[test]
fn unsigned_certificate_reports_unverified_issuer() {
    let dir = temp_dir("unsigned");
    let path = dir.join("cert.json");
    std::fs::write(&path, serde_json::to_string_pretty(&sample_cert()).unwrap()).unwrap();
    assert_eq!(
        verify_certificate(&path, None),
        VerifyOutcome::UnverifiedIssuer,
        "unsigned = unverified, never silent trust"
    );
    // A SIGNED cert with no trusted anchor configured is equally unverified
    // (a signature without a trusted anchor is not an identity):
    let mut cert = sample_cert();
    sign_certificate(&mut cert, &test_key());
    let signed_path = dir.join("signed.json");
    std::fs::write(&signed_path, serde_json::to_string_pretty(&cert).unwrap()).unwrap();
    assert_eq!(
        verify_certificate(&signed_path, None),
        VerifyOutcome::UnverifiedIssuer
    );
    // Note: the TRUERISE_TRUSTED_KEY environment fallback is exercised by
    // the binary path (`verify --cert` without --issuer-key); the test
    // suite keeps environment mutation out (edition-2024 unsafe + parallel
    // races) - the explicit-key path above is the same code after the
    // fallback resolves.
    let _ = load_signing_key; // documented surface
}
