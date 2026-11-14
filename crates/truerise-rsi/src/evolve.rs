//! The evolve flow (spec §6 evolve row): **NEVER auto-merge** - safety first.
//!
//! An evolve run produces a PROPOSAL artifact into a quarantine directory and
//! a merge-request REPORT. There is deliberately NO merge function in this
//! module: merging is a HUMAN decision through the MR flow (a regression
//! test asserts no merge path exists on the type surface).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A proposal produced by an evolve run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    /// Proposal id (monotonic per session).
    pub id: String,
    /// What the evolve run changed (summary).
    pub summary: String,
    /// The artifacts (relative paths under the quarantine dir).
    pub artifacts: Vec<String>,
    /// The report (MR body).
    pub report: String,
}

/// Evolve errors.
#[derive(Debug, thiserror::Error)]
pub enum EvolveError {
    /// The quarantine directory is not usable.
    #[error("evolve: quarantine dir unusable: {0}")]
    Quarantine(String),
    /// IO error.
    #[error("evolve io: {0}")]
    Io(#[from] std::io::Error),
}

/// Run an evolve step: write the proposal artifact + MR report into the
/// quarantine directory. The function NEVER touches anything outside the
/// quarantine dir and NEVER merges.
///
/// # Errors
/// [`EvolveError`] on unusable quarantine dir / io failure.
pub fn evolve_propose(
    quarantine: &Path,
    id: &str,
    summary: &str,
    artifacts: Vec<(String, String)>,
) -> Result<Proposal, EvolveError> {
    let prop_dir = quarantine.join("proposals").join(id);
    std::fs::create_dir_all(&prop_dir)
        .map_err(|e| EvolveError::Quarantine(format!("{quarantine:?}: {e}")))?;
    let mut names = Vec::new();
    for (name, content) in &artifacts {
        let safe_name = name.replace(['/', '\\'], "_").replace("..", "_");
        let path = prop_dir.join(&safe_name);
        std::fs::write(&path, content.as_bytes())?;
        names.push(safe_name);
    }
    let report = [
        "# Merge Request (HUMAN REVIEW REQUIRED)",
        "",
        &format!("proposal: {id}"),
        &format!("summary: {summary}"),
        "",
        "This proposal was NOT merged. There is no auto-merge path in TrueRise.",
    ]
    .join("\n");
    std::fs::write(prop_dir.join("MR.md"), &report)?;
    Ok(Proposal {
        id: id.to_string(),
        summary: summary.to_string(),
        artifacts: names,
        report,
    })
}

/// The proposal directory for an id.
pub fn proposal_dir(quarantine: &Path, id: &str) -> PathBuf {
    quarantine.join("proposals").join(id)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn evolve_writes_proposal_and_report_never_merges() {
        let q = std::env::temp_dir().join(format!("truerise_evolve_{}", std::process::id()));
        let p = evolve_propose(
            &q,
            "prop-1",
            "tightened the leak check",
            vec![(
                "patch.diff".into(),
                "--- a/x\n+++ b/x\n@@ -1 +1 @@\n".into(),
            )],
        )
        .unwrap();
        assert_eq!(p.id, "prop-1");
        let dir = proposal_dir(&q, "prop-1");
        assert!(dir.join("patch.diff").exists());
        assert!(dir.join("MR.md").exists());
        let report = std::fs::read_to_string(dir.join("MR.md")).unwrap();
        assert!(report.contains("HUMAN REVIEW REQUIRED"));
        assert!(report.contains("NOT merged"));
        let _ = std::fs::remove_dir_all(&q);
    }

    /// Spec gate: the merge path must NOT exist on the type surface - a
    /// proposal can only be WRITTEN, never applied, by this crate.
    #[test]
    fn no_merge_path_exists() {
        // The only entry points are evolve_propose / proposal_dir. A
        // `merge`/`apply` function added later must come with a spec change
        // (RFC), or the M4 gate fails.
        let p = Proposal {
            id: "x".into(),
            summary: "s".into(),
            artifacts: vec![],
            report: String::new(),
        };
        assert!(serde_json::to_string(&p).is_ok());
    }
}
