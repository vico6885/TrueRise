//! The ratchet baseline resolution + the verdict-to-exit-code mapping
//! (shared by the run path and the evolve path).

use std::path::{Path, PathBuf};

/// The default ratchet baseline: `baselines/run_scores.json` at this repo
/// root (resolved by walking up from the cwd, like git does; `None` when no
/// ancestor carries it - the judge then runs with an empty baseline and the
/// verdict row says so).
#[must_use]
pub fn find_default_baseline() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join("baselines").join("run_scores.json");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Load a ratchet baseline from a JSON file (integer metric values; the
/// `_note` key and any non-numeric field is ignored).
///
/// Review fix (P2, fail-open removal): an EXPLICIT baseline path that
/// cannot be read or parsed used to silently degrade to an empty baseline
/// (the ratchet gate vanished, the terminal banner still said "loaded").
/// Explicit = load-or-error; only the ABSENT default is empty.
pub fn load_baseline(path: Option<&Path>) -> Result<truerise_eval::ratchet::Baseline, String> {
    let Some(path) = path else {
        return Ok(truerise_eval::ratchet::Baseline::new());
    };
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("baseline {} unreadable: {e}", path.display()))?;
    let doc: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("baseline {} is not valid JSON: {e}", path.display()))?;
    let Some(obj) = doc.as_object() else {
        return Err(format!("baseline {} is not a JSON object", path.display()));
    };
    Ok(obj
        .iter()
        .filter_map(|(k, v)| v.as_f64().map(|x| (k.clone(), x)))
        .collect())
}

/// Map a judged verdict onto the exit code (G7 contract: PASS=0 / FAIL=1 /
/// INCONCLUSIVE=3 - a run that proves nothing gets its OWN code, never 0,
/// never 1; CI lanes still go red on it, and it can never read as either
/// verdict).
#[must_use]
pub fn grade_exit(verdict: &truerise_eval::RunVerdict) -> i32 {
    if verdict.is_pass() {
        0
    } else if verdict.verdict_str() == "INCONCLUSIVE" {
        3
    } else {
        1
    }
}
