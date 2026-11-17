//! Certificate signing identity (W10): the "Verifiable" obligation's second
//! half - a stranger can recompute offline AND verify WHO issued the
//! certificate. Ed25519; the signing key lives in the environment ONLY
//! (never in the repo, argv, or logs). An unsigned certificate reports
//! UNVERIFIED ISSUER - it is never silently trusted.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::{Value, json};

/// The environment variable carrying the hex signing seed (32 bytes).
pub const SIGNING_KEY_ENV: &str = "TRUERISE_SIGNING_KEY";
/// The environment variable carrying the hex trusted issuer key (32 bytes).
pub const TRUSTED_KEY_ENV: &str = "TRUERISE_TRUSTED_KEY";

/// Load the signing key from the environment (hex seed).
#[must_use]
pub fn load_signing_key() -> Option<SigningKey> {
    let hex = std::env::var(SIGNING_KEY_ENV).ok()?;
    // Review fix (P2): a seed longer than 64 hex used to be silently
    // TRUNCATED to its first 32 bytes - wrong-key signatures with no
    // signal. Fail-closed: exactly 64 hex or nothing.
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let seed: Vec<u8> = (0..32)
        .map(|i| u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    let seed: [u8; 32] = seed.try_into().ok()?;
    Some(SigningKey::from_bytes(&seed))
}

/// The canonical signing payload: the certificate WITHOUT its signature
/// field, serialized with sorted keys (serde_json Map is a BTreeMap by
/// default - deterministic across runs).
#[must_use]
pub fn signing_payload(cert: &Value) -> String {
    let mut bare = cert.clone();
    if let Some(obj) = bare.as_object_mut() {
        obj.remove("signature");
    }
    serde_json::to_string(&bare).unwrap_or_default()
}

/// Sign ``cert`` in place (adds the `signature` field: scheme + public key
/// + hex signature over the signing payload).
pub fn sign_certificate(cert: &mut Value, key: &SigningKey) {
    let payload = signing_payload(cert);
    let sig = key.sign(payload.as_bytes());
    cert["signature"] = json!({
        "scheme": "ed25519",
        "public_key": hex_encode(&key.verifying_key().to_bytes()),
        "signature": hex_encode(&sig.to_bytes())
    });
}

/// The outcome of a verification attempt (an unsigned cert is NEVER treated
/// as verified).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// Signature valid under the TRUSTED issuer key.
    Verified {
        /// The issuer fingerprint (hex of the verifying key).
        issuer: String,
    },
    /// No signature field - the issuer is UNKNOWN (never trusted).
    UnverifiedIssuer,
    /// A signature exists but does not verify (tampered or wrong key).
    InvalidSignature,
    /// The file is not a parsable certificate.
    Unparsable,
}

/// Verify a certificate file against an optional TRUSTED issuer key (hex;
/// falls back to [`TRUSTED_KEY_ENV`]). `None` trusted key -> UnverifiedIssuer
/// even when a signature exists (a signature without a trusted anchor is
/// not an identity).
#[must_use]
pub fn verify_certificate(path: &std::path::Path, trusted_key_hex: Option<&str>) -> VerifyOutcome {
    let Ok(text) = std::fs::read_to_string(path) else {
        return VerifyOutcome::Unparsable;
    };
    let Ok(cert) = serde_json::from_str::<Value>(&text) else {
        return VerifyOutcome::Unparsable;
    };
    let Some(sig_obj) = cert.get("signature") else {
        return VerifyOutcome::UnverifiedIssuer;
    };
    let (Some(scheme), Some(public_hex), Some(sig_hex)) = (
        sig_obj.get("scheme").and_then(Value::as_str),
        sig_obj.get("public_key").and_then(Value::as_str),
        sig_obj.get("signature").and_then(Value::as_str),
    ) else {
        return VerifyOutcome::Unparsable;
    };
    if scheme != "ed25519" {
        return VerifyOutcome::Unparsable;
    }
    let trusted_hex = trusted_key_hex
        .map(str::to_string)
        .or_else(|| std::env::var(TRUSTED_KEY_ENV).ok());
    let Some(trusted_hex) = trusted_hex else {
        return VerifyOutcome::UnverifiedIssuer;
    };
    let Some(trusted) = hex_decode_32(&trusted_hex) else {
        return VerifyOutcome::UnverifiedIssuer;
    };
    let Ok(verifying) = VerifyingKey::from_bytes(&trusted) else {
        return VerifyOutcome::UnverifiedIssuer;
    };
    let Some(sig_bytes) = hex_decode_64(sig_hex) else {
        return VerifyOutcome::InvalidSignature;
    };
    let Ok(signature) = Signature::from_slice(&sig_bytes) else {
        return VerifyOutcome::InvalidSignature;
    };
    // The public key IN the cert must match the trusted key (a cert signed
    // by another key is not ours, whatever it claims).
    let Some(cert_key) = hex_decode_32(public_hex) else {
        return VerifyOutcome::InvalidSignature;
    };
    if cert_key != trusted {
        return VerifyOutcome::InvalidSignature;
    }
    match verifying.verify(signing_payload(&cert).as_bytes(), &signature) {
        Ok(()) => VerifyOutcome::Verified {
            issuer: trusted_hex.to_lowercase(),
        },
        Err(_) => VerifyOutcome::InvalidSignature,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode_32(hex: &str) -> Option<[u8; 32]> {
    decode(hex).and_then(|v| <[u8; 32]>::try_from(v).ok())
}

fn hex_decode_64(hex: &str) -> Option<Vec<u8>> {
    decode(hex)
}

fn decode(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}
