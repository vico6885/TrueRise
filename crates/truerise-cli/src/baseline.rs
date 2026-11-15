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
pub fn load_baseline(path: Option<&Path>) -> truerise_eval::ratchet::Baseline {
    let Some(path) = path else {
        return truerise_eval::ratchet::Baseline::new();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return truerise_eval::ratchet::Baseline::new();
    };
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&text) else {
        return truerise_eval::ratchet::Baseline::new();
    };
    doc.as_object()
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_f64().map(|x| (k.clone(), x)))
                .collect()
        })
        .unwrap_or_else(truerise_eval::ratchet::Baseline::new)
}

/// Map a judged verdict onto the exit code (PASS=0 / FAIL=1 - the same
/// contract as the demo; no third meaning is ever introduced).
#[must_use]
pub fn grade_exit(verdict: &truerise_eval::RunVerdict) -> i32 {
    i32::from(!verdict.is_pass())
}
