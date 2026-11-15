//! The OTel tier (W15, feature `otel`): audit rows become REAL
//! OpenTelemetry spans. The mapping is one-way (the chain is the
//! authority; spans are a projection) and exporter-agnostic - the stdout
//! exporter ships in the feature for determinism; wire an OTLP exporter at
//! the boundary when a collector exists. Sensitive keys are redacted on
//! the way out (the SAME redact rules as the jsonl tier).

#![cfg(feature = "otel")]

use std::path::Path;

use opentelemetry::trace::{Span as _, Tracer, TracerProvider as _};
use opentelemetry_sdk::trace::TracerProvider;

use crate::redact;

/// Export every chain row as one OTel span through the stdout exporter.
///
/// No global tracer state is touched: the provider is local to this call
/// (embedding hosts keep their own pipeline; this projection is one-way).
///
/// # Errors
/// IO / parse errors surface as strings at the port boundary.
pub fn export_spans_stdout(chain_path: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(chain_path).map_err(|e| e.to_string())?;
    let rows: Vec<serde_json::Value> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, String>>()?;
    let count = rows_len(&text);
    let provider = TracerProvider::builder()
        .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
        .build();
    let tracer = provider.tracer("truerise.observe");
    for row in rows {
        let detail = redact(row.get("detail").unwrap_or(&serde_json::Value::Null));
        let kind = row.get("kind").and_then(|k| k.as_str()).unwrap_or("row");
        let seq = row.get("seq").and_then(|s| s.as_u64()).unwrap_or(0);
        let severity = row
            .get("severity")
            .and_then(|s| s.as_str())
            .unwrap_or("info")
            .to_string();
        let mut span = tracer
            .span_builder(format!("truerise.{kind}"))
            .with_attributes(vec![
                opentelemetry::KeyValue::new("truerise.seq", seq as i64),
                opentelemetry::KeyValue::new("truerise.severity", severity.clone()),
                opentelemetry::KeyValue::new("truerise.detail", detail.to_string()),
            ])
            .start(&tracer);
        span.end();
    }
    let _ = provider.shutdown();
    Ok(count)
}

fn rows_len(text: &str) -> usize {
    text.lines().filter(|l| !l.trim().is_empty()).count()
}
