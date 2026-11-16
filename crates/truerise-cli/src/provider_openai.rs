//! The OpenAI-compatible `ModelRouter` adapter (W14): ONE adapter covers
//! every provider that speaks the de-facto standard chat-completions shape
//! (OpenAI, vLLM, Ollama, DeepSeek, Qwen, Together, ...). Keys come from
//! the environment ONLY (SECURITY.md); temperature is pinned to 0 and the
//! adapter self-reports the requested model as the response model so round
//! pinning can void foreign answers.

use truerise_core::{ModelRouter, RouterError, TurnRequest, TurnResponse};

/// The environment variable read when no explicit key is configured.
pub const API_KEY_ENV: &str = "TRUERISE_API_KEY";

/// An OpenAI-compatible chat-completions router.
pub struct OpenAICompatRouter {
    /// e.g. `https://api.openai.com/v1` or `http://127.0.0.1:8080/v1`.
    base_url: String,
    /// Bearer key; `None` only makes sense for local servers.
    api_key: Option<String>,
    /// The requested model id (self-reported back for pinning).
    model: String,
}

impl OpenAICompatRouter {
    /// Build an adapter. `api_key: None` reads [`API_KEY_ENV`].
    #[must_use]
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: std::env::var(API_KEY_ENV).ok(),
            model: model.to_string(),
        }
    }

    /// Build with an EXPLICIT key source name (for tests / local servers).
    #[must_use]
    pub fn with_key(base_url: &str, model: &str, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model: model.to_string(),
        }
    }
}

impl ModelRouter for OpenAICompatRouter {
    fn route(&self, request: &TurnRequest) -> Result<TurnResponse, RouterError> {
        let body = serde_json::json!({
            "model": self.model,
            // Determinism rail: temperature 0 (the tape is the real replay).
            "temperature": 0,
            "messages": [ { "role": "user", "content": request.payload } ]
        });
        let url = format!("{}/chat/completions", self.base_url);
        let mut req = ureq::post(&url)
            .timeout(std::time::Duration::from_secs(120))
            .set("Content-Type", "application/json");
        if let Some(key) = &self.api_key {
            req = req.set("Authorization", &format!("Bearer {key}"));
        }
        let response = req.send_json(&body).map_err(|e| match e {
            ureq::Error::Status(code, _) => crate::provider_common::classify_status(code),
            _other => RouterError::Unavailable,
        })?;
        let doc: serde_json::Value = response
            .into_json()
            .map_err(|e| RouterError::Fatal(format!("provider body: {e}")))?;
        let content = doc
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| {
                RouterError::Fatal("provider response: missing choices[0].message.content".into())
            })?;
        // The provider's OWN echo of the model id backs the pinning check.
        let model = doc
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(&self.model)
            .to_string();
        Ok(TurnResponse {
            model,
            payload: content.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A canned chat-completions server: asserts the auth header + body
    /// shape, answers one request, exits.
    fn canned_server(
        expected_auth: Option<String>,
        reply: &'static str,
    ) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            // Read until end of headers + body (simple canned protocol:
            // read headers, find content-length, read that many bytes).
            let mut headers = String::new();
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
            buf.resize(len, 0);
            stream.read_exact(&mut buf).expect("body");
            let body = String::from_utf8_lossy(&buf);
            assert!(
                body.contains("\"temperature\":0"),
                "temperature pinned: {body}"
            );
            match expected_auth {
                Some(key) => assert!(
                    headers
                        .to_lowercase()
                        .contains(&format!("authorization: bearer {key}")),
                    "auth header present"
                ),
                None => assert!(
                    !headers.to_lowercase().contains("authorization:"),
                    "no phantom auth header"
                ),
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
            stream.flush().expect("flush");
        });
        (addr, handle)
    }

    const REPLY: &str = r#"{"model":"test-model","choices":[{"message":{"role":"assistant","content":"canned answer"}}]}"#;

    #[test]
    fn openai_compat_adapter_speaks_the_standard_shape() {
        let (addr, handle) = canned_server(Some("test-key-123".to_string()), REPLY);
        let router = OpenAICompatRouter::with_key(
            &format!("http://{addr}/v1"),
            "test-model",
            Some("test-key-123".into()),
        );
        let response = router
            .route(&TurnRequest {
                payload: "hello".into(),
                tag: "plan".into(),
            })
            .expect("route");
        assert_eq!(
            response.model, "test-model",
            "self-reports the requested model"
        );
        assert_eq!(response.payload, "canned answer");
        handle.join().expect("server thread");
    }

    #[test]
    fn missing_key_never_sends_a_phantom_header() {
        let (addr, handle) = canned_server(None, REPLY);
        let router = OpenAICompatRouter::with_key(&format!("http://{addr}/v1"), "m", None);
        let response = router.route(&TurnRequest {
            payload: "x".into(),
            tag: "plan".into(),
        });
        assert!(response.is_ok());
        handle.join().expect("server thread");
    }
}
