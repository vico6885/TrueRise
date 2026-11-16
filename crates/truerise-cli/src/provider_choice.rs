//! Provider selection (ROADMAP item 4): which external model backs a run,
//! resolved fail-closed from the environment.

use crate::provider_anthropic;
use crate::provider_openai;

/// Which external provider a run uses (the model id is REQUIRED with it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderKind {
    /// Any chat-completions endpoint (OpenAI, vLLM, Ollama, DeepSeek, ...).
    OpenAICompat,
    /// The Anthropic Messages API.
    Anthropic,
}

/// A fully specified external provider choice (keys come from the
/// ENVIRONMENT only - argv is forbidden by SECURITY.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSpec {
    /// Which API shape to speak.
    pub kind: ProviderKind,
    /// Base URL override (local vLLM/Ollama/proxy); `None` = the vendor default.
    pub url: Option<String>,
    /// The model id to request AND pin the run to.
    pub model: String,
}

/// Build the router for a provider choice; fails closed when the key is
/// not in the environment (never argv, never config).
pub fn build_provider_router(
    spec: &ProviderSpec,
) -> Result<(Box<dyn truerise_core::ModelRouter>, String), String> {
    let (default_url, key_env) = match spec.kind {
        ProviderKind::OpenAICompat => ("https://api.openai.com/v1", provider_openai::API_KEY_ENV),
        ProviderKind::Anthropic => (
            "https://api.anthropic.com",
            provider_anthropic::ANTHROPIC_KEY_ENV,
        ),
    };
    let key = std::env::var(key_env).map_err(|_| {
        format!(
            "provider {} needs {key_env} in the environment (keys never enter argv or config)",
            match spec.kind {
                ProviderKind::OpenAICompat => "openai-compatible",
                ProviderKind::Anthropic => "anthropic",
            }
        )
    })?;
    let url = spec.url.clone().unwrap_or_else(|| default_url.to_string());
    let router: Box<dyn truerise_core::ModelRouter> = match spec.kind {
        ProviderKind::OpenAICompat => Box::new(provider_openai::OpenAICompatRouter::with_key(
            &url,
            &spec.model,
            Some(key),
        )),
        ProviderKind::Anthropic => Box::new(provider_anthropic::AnthropicRouter::with_key(
            &url,
            &spec.model,
            Some(key),
        )),
    };
    Ok((router, spec.model.clone()))
}

/// Parse `--provider <openai|anthropic>`, `--model <id>`,
/// `--provider-url <u>` out of the CLI args. `Err` = usage error message
/// (the caller prints it and exits 2); `--model` is REQUIRED with
/// `--provider` (the run pins to it).
///
/// # Errors
/// String-typed usage messages at the CLI boundary.
pub fn parse_provider_args(args: &[String]) -> Result<Option<ProviderSpec>, String> {
    let mut kind: Option<ProviderKind> = None;
    let mut url: Option<String> = None;
    let mut model: Option<String> = None;
    let mut idx = 0;
    while idx < args.len() {
        let arg = args[idx].as_str();
        match arg {
            "--provider" => match args.get(idx + 1).map(String::as_str) {
                Some("openai") => {
                    kind = Some(ProviderKind::OpenAICompat);
                    idx += 2;
                }
                Some("anthropic") => {
                    kind = Some(ProviderKind::Anthropic);
                    idx += 2;
                }
                Some(other) => {
                    return Err(format!("unknown provider {other} (openai | anthropic)"));
                }
                None => return Err("--provider needs openai | anthropic".into()),
            },
            "--provider-url" => match args.get(idx + 1) {
                Some(u) => {
                    url = Some(u.clone());
                    idx += 2;
                }
                None => return Err("--provider-url needs a base URL".into()),
            },
            "--model" => match args.get(idx + 1) {
                Some(m) => {
                    model = Some(m.clone());
                    idx += 2;
                }
                None => return Err("--model needs a model id".into()),
            },
            _ => idx += 1,
        }
    }
    match kind {
        None => Ok(None),
        Some(k) => match model {
            Some(m) => Ok(Some(ProviderSpec {
                kind: k,
                url,
                model: m,
            })),
            None => Err("--provider needs --model <id> (the run pins to it)".into()),
        },
    }
}
