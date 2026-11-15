//! `truerise_native` (W13b): the PyO3 backend for the `truerise` Python
//! package. THIN bindings only - every behaviour lives in the workspace
//! crates (the judging/wire authorities stay unique); this module just
//! makes them importable from Python. The pure-Python SDK remains the
//! fallback and the contract reference: `truerise.sdk.native_available()`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use truerise_governance::HashChain;
use truerise_wire::Severity;

fn severity_from(name: &str) -> Result<Severity, PyErr> {
    match name {
        "debug" => Ok(Severity::Debug),
        "info" => Ok(Severity::Info),
        "warn" => Ok(Severity::Warn),
        "error" => Ok(Severity::Error),
        other => Err(PyValueError::new_err(format!("unknown severity: {other}"))),
    }
}

fn chain_error(err: truerise_governance::ChainError) -> PyErr {
    PyValueError::new_err(err.to_string())
}

/// Append one row to a persisted hash chain; returns the row's hash.
#[pyfunction]
#[pyo3(signature = (path, kind, detail_json, severity = "info", replay_key = None))]
fn chain_append(
    path: &str,
    kind: &str,
    detail_json: &str,
    severity: &str,
    replay_key: Option<&str>,
) -> PyResult<String> {
    let detail: serde_json::Value = serde_json::from_str(detail_json)
        .map_err(|e| PyValueError::new_err(format!("detail unparsable: {e}")))?;
    let chain = HashChain::open(std::path::Path::new(path)).map_err(chain_error)?;
    let row = chain
        .append(kind, severity_from(severity)?, &detail, replay_key)
        .map_err(chain_error)?;
    Ok(row.event.hash)
}

/// Verify a persisted chain file; raises on tamper (fail-closed).
#[pyfunction]
fn chain_verify(path: &str) -> PyResult<usize> {
    let chain = HashChain::open(std::path::Path::new(path)).map_err(chain_error)?;
    chain.verify().map_err(chain_error)?;
    Ok(chain.len())
}

/// The tighten-only ratchet over INTEGER metrics (per-mille / counts -
/// the float rule). Returns the active baseline; raises on loosening.
#[pyfunction]
fn ratchet_propose(baseline_json: &str, candidate_json: &str) -> PyResult<String> {
    let baseline: std::collections::BTreeMap<String, f64> = serde_json::from_str(baseline_json)
        .map_err(|e| PyValueError::new_err(format!("baseline unparsable: {e}")))?;
    let candidate: std::collections::BTreeMap<String, f64> =
        serde_json::from_str(candidate_json)
            .map_err(|e| PyValueError::new_err(format!("candidate unparsable: {e}")))?;
    let mut ratchet = truerise_eval::Ratchet::new(baseline);
    ratchet
        .propose(&candidate)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    serde_json::to_string(ratchet.baseline())
        .map_err(|e| PyValueError::new_err(format!("baseline unserialisable: {e}")))
}

/// Verify scores against the baseline; raises on regression (exit-grade).
#[pyfunction]
fn ratchet_verify(baseline_json: &str, scores_json: &str) -> PyResult<()> {
    let baseline: std::collections::BTreeMap<String, f64> = serde_json::from_str(baseline_json)
        .map_err(|e| PyValueError::new_err(format!("baseline unparsable: {e}")))?;
    let scores: std::collections::BTreeMap<String, f64> = serde_json::from_str(scores_json)
        .map_err(|e| PyValueError::new_err(format!("scores unparsable: {e}")))?;
    truerise_eval::Ratchet::new(baseline)
        .verify(&scores)
        .map_err(|e| PyValueError::new_err(e.to_string()))
}

/// The Python module.
#[pymodule]
fn truerise_native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(chain_append, m)?)?;
    m.add_function(wrap_pyfunction!(chain_verify, m)?)?;
    m.add_function(wrap_pyfunction!(ratchet_propose, m)?)?;
    m.add_function(wrap_pyfunction!(ratchet_verify, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    // The binding layer is exercised from Python (pytest skips when the
    // native module is absent); the Rust-side logic it binds is already
    // covered by the workspace crates' own suites.
    #[test]
    fn binding_surface_compiles() {
        // A compile-time placeholder: the functions above ARE the surface.
    }
}
