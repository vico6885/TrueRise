//! The Anthropic Messages API adapter (ROADMAP item 4): a second
//! `ModelRouter` behind the same port. Shape:
//! POST `{base_url}/v1/messages`, headers `x-api-key` +
//! `anthropic-version`, body with REQUIRED `max_tokens`, temperature
//! pinned to 0. Keys come from the environment ONLY (`ANTHROPIC_API_KEY`;
//! argv is forbidden by SECURITY.md). The response's own `model` field is
//! self-reported back, so the round-pinning check catches silent model
//! substitution.

use truerise_core::{ModelRouter, RouterError, TurnRequest, TurnResponse};

/// The environment variable carrying the Anthropic key.
pub const ANTHROPIC_KEY_ENV: &str = "ANTHROPIC_API_KEY";
/// The pinned API version header value.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// An Anthropic Messages API router.
pub struct AnthropicRouter {
    /// e.g. `https://api.anthropic.com` (a local proxy also works).
    base_url: String,
    api_key: Option<String>,
    model: String,
    max_tokens: u32,
}

impl AnthropicRouter {
    /// Build an adapter; the key is read from [`ANTHROPIC_KEY_ENV`].
    #[must_use]
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: std::env::var(ANTHROPIC_KEY_ENV).ok(),
            model: model.to_string(),
            max_tokens: 1024,
        }
    }

    /// Build with an EXPLICIT key (tests / local servers).
    #[must_use]
    pub fn with_key(base_url: &str, model: &str, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model: model.to_string(),
            max_tokens: 1024,
        }
    }
}

impl ModelRouter for AnthropicRouter {
    fn route(&self, request: &TurnRequest) -> Result<TurnResponse, RouterError> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": self.max_tokens,
            // Determinism rail: temperature 0 (the tape is the real replay).
            "temperature": 0,
            "messages": [ { "role": "user", "content": request.payload } ]
        });
        let url = format!("{}/v1/messages", self.base_url);
        let mut req = ureq::post(&url)
            .timeout(std::time::Duration::from_secs(120))
            .set("Content-Type", "application/json")
            .set("anthropic-version", ANTHROPIC_VERSION);
        if let Some(key) = &self.api_key {
            req = req.set("x-api-key", key);
        }
        let response = req.send_json(&body).map_err(|e| match e {
            ureq::Error::Status(code, _) => crate::provider_common::classify_status(code),
            _ => RouterError::Unavailable,
        })?;
        let doc: serde_json::Value = response
            .into_json()
            .map_err(|e| RouterError::Fatal(format!("provider body: {e}")))?;
        let text = doc
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                RouterError::Fatal("provider response: missing content[0].text".into())
            })?;
        // The provider's OWN echo of the model id backs the pinning check.
        let model = doc
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(&self.model)
            .to_string();
        Ok(TurnResponse {
            model,
            payload: text.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const REPLY: &str =
        r#"{"model":"test-model","content":[{"type":"text","text":"canned answer"}]}"#;

    /// Canned Messages-API server: asserts header/body shape, answers N
    /// sequential requests with queued replies.
    fn canned_server(
        expected_key: Option<String>,
        replies: Vec<&'static str>,
    ) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut headers = String::new();
                let mut byte = [0u8; 1];
                loop {
                    let n = stream.read(&mut byte).expect("read");
                    if n == 0 {
                        break;
                    }
                    headers.push(byte[0] as char);
                    if headers.ends_with("\r\n\r\n") {
                        break;
                    }
                }
                let len: usize = headers
                    .to_lowercase()
                    .lines()
                    .find_map(|l| {
                        l.strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap_or(0))
                    })
                    .unwrap_or(0);
                let mut buf = vec![0u8; len];
                stream.read_exact(&mut buf).expect("body");
                let body = String::from_utf8_lossy(&buf);
                assert!(
                    body.contains("\"temperature\":0"),
                    "temperature pinned: {body}"
                );
                assert!(
                    body.contains("\"max_tokens\""),
                    "max_tokens is REQUIRED: {body}"
                );
                assert!(body.contains("\"model\""), "model is REQUIRED: {body}");
                match &expected_key {
                    Some(key) => assert!(
                        headers
                            .to_lowercase()
                            .contains(&format!("x-api-key: {key}")),
                        "x-api-key header present"
                    ),
                    None => assert!(
                        !headers.to_lowercase().contains("x-api-key:"),
                        "no phantom key header"
                    ),
                }
                assert!(
                    headers.contains(&format!("anthropic-version: {ANTHROPIC_VERSION}")),
                    "version header present"
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                stream.write_all(response.as_bytes()).expect("write");
                stream.flush().expect("flush");
            }
        });
        (addr, handle)
    }

    #[test]
    fn anthropic_adapter_speaks_the_messages_shape() {
        let (addr, handle) = canned_server(Some("test-key".into()), vec![REPLY, REPLY, REPLY]);
        let router = AnthropicRouter::with_key(
            &format!("http://{addr}"),
            "test-model",
            Some("test-key".into()),
        );
        for tag in ["plan", "act", "observe"] {
            let response = router
                .route(&TurnRequest {
                    payload: format!("{tag} payload"),
                    tag: tag.into(),
                })
                .expect("route");
            assert_eq!(
                response.model, "test-model",
                "the provider's own echo backs pinning"
            );
            assert_eq!(response.payload, "canned answer");
        }
        handle.join().expect("server thread");
    }

    #[test]
    fn anthropic_missing_key_never_sends_phantom_header() {
        let (addr, handle) = canned_server(None, vec![REPLY]);
        let router = AnthropicRouter::with_key(&format!("http://{addr}"), "m", None);
        router
            .route(&TurnRequest {
                payload: "x".into(),
                tag: "plan".into(),
            })
            .expect("route without a key still works (local servers)");
        handle.join().expect("server thread");
    }
}
