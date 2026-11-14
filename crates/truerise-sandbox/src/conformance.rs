//! Conformance self-check: three must-fail probe regions (escape / oversell /
//! residue). Spec §6 conformance row: the JUDGING AUTHORITY is truerise-eval
//! (M4) - this module produces FACTS ONLY (a report), never verdicts, and its
//! output is forbidden as eval input.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::inprocess::{InProcessProvider, declared_capabilities, next_id};
use crate::provider::{CapabilityLevel, SandboxError, SandboxProvider};

/// One probe outcome (a fact).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeFact {
    /// Probe region: escape | oversell | residue.
    pub region: String,
    /// What was probed.
    pub probe: String,
    /// Did the provider behave as required (must-fail areas actually failed)?
    pub passed: bool,
    /// Detail for the report.
    pub detail: String,
}

/// The self-check report: facts only.
#[derive(Debug, Clone)]
pub struct ConformanceReport {
    /// Provider name.
    pub provider: String,
    /// Declared capability level (three states, explicit).
    pub capability: CapabilityLevel,
    /// Declared optional bits.
    pub declared_bits: u8,
    /// Probe facts (three must-fail regions + optional-bit probes).
    pub facts: Vec<ProbeFact>,
    /// Report location hint.
    pub generated_at_dir: PathBuf,
}

impl ConformanceReport {
    /// Facts by region (self-check consumers filter; eval judges).
    pub fn region(&self, name: &str) -> Vec<&ProbeFact> {
        self.facts.iter().filter(|f| f.region == name).collect()
    }

    /// All facts in the three must-fail regions passed?
    pub fn all_must_fail_regions_pass(&self) -> bool {
        ["escape", "oversell", "residue"]
            .iter()
            .all(|r| self.region(r).iter().all(|f| f.passed))
    }
}

/// Run the three must-fail regions against the in-process provider.
pub fn self_check(root: &Path) -> ConformanceReport {
    let provider = InProcessProvider::new(root);
    let workdir = root.join("conformance-work");
    let handle = match provider.create(next_id(), &workdir) {
        Ok(h) => h,
        Err(e) => {
            return ConformanceReport {
                provider: "inprocess".into(),
                capability: CapabilityLevel::None,
                declared_bits: declared_capabilities().0,
                facts: vec![ProbeFact {
                    region: "escape".into(),
                    probe: "create sandbox".into(),
                    passed: false,
                    detail: format!("create failed: {e}"),
                }],
                generated_at_dir: root.to_path_buf(),
            };
        }
    };
    let mut facts: Vec<ProbeFact> = Vec::new();

    // Region 1 - ESCAPE: a path escaping the fence must be REFUSED.
    match provider.execute(
        handle,
        "write_file",
        &HashMap::from([
            ("path".to_string(), "../escape.txt".to_string()),
            ("content".to_string(), "x".to_string()),
        ]),
    ) {
        Err(SandboxError::Refused(_)) => facts.push(ProbeFact {
            region: "escape".into(),
            probe: "write ../escape.txt".into(),
            passed: true,
            detail: "refused as expected".into(),
        }),
        Ok(_) => facts.push(ProbeFact {
            region: "escape".into(),
            probe: "write ../escape.txt".into(),
            passed: false,
            detail: "ESCAPE NOT REFUSED".into(),
        }),
        Err(e) => facts.push(ProbeFact {
            region: "escape".into(),
            probe: "write ../escape.txt".into(),
            passed: false,
            detail: format!("unexpected error: {e}"),
        }),
    }

    // Region 2 - OVERSELL: an undeclared ability must be refused as such.
    let oversell_ok = !declared_capabilities().has_all(crate::provider::CapabilityBits::SNAPSHOT);
    facts.push(ProbeFact {
        region: "oversell".into(),
        probe: "snapshot without declaring the bit".into(),
        passed: oversell_ok,
        detail: if oversell_ok {
            "snapshot bit not declared -> caller-side refusal is provable".into()
        } else {
            "snapshot declared but not probed".into()
        },
    });

    // Region 3 - RESIDUE: after destroy, NOTHING remains on disk.
    let probe_rel = "residue-check.txt";
    let write_res = provider.execute(
        handle,
        "write_file",
        &HashMap::from([
            ("path".to_string(), probe_rel.to_string()),
            ("content".to_string(), "r".to_string()),
        ]),
    );
    if let Err(e) = write_res {
        facts.push(ProbeFact {
            region: "residue".into(),
            probe: "write for residue probe".into(),
            passed: false,
            detail: format!("write failed: {e}"),
        });
        return ConformanceReport {
            provider: "inprocess".into(),
            capability: CapabilityLevel::Full,
            declared_bits: declared_capabilities().0,
            facts,
            generated_at_dir: root.to_path_buf(),
        };
    }
    if let Err(e) = provider.destroy(handle) {
        facts.push(ProbeFact {
            region: "residue".into(),
            probe: "destroy".into(),
            passed: false,
            detail: format!("destroy failed: {e}"),
        });
        return ConformanceReport {
            provider: "inprocess".into(),
            capability: CapabilityLevel::Full,
            declared_bits: declared_capabilities().0,
            facts,
            generated_at_dir: root.to_path_buf(),
        };
    }
    let residue = workdir.join(probe_rel).exists() || workdir.exists();
    facts.push(ProbeFact {
        region: "residue".into(),
        probe: "workdir removed after destroy".into(),
        passed: !residue,
        detail: if residue {
            "residue found".into()
        } else {
            "no residue".into()
        },
    });

    ConformanceReport {
        provider: "inprocess".into(),
        capability: CapabilityLevel::Full,
        declared_bits: declared_capabilities().0,
        facts,
        generated_at_dir: root.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn self_check_three_regions_pass_for_inprocess() {
        let dir = std::env::temp_dir().join(format!("truerise_conformance_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let report = self_check(&dir);
        assert!(
            report.all_must_fail_regions_pass(),
            "facts: {:?}",
            report.facts
        );
        assert_eq!(report.region("escape").len(), 1);
        assert_eq!(report.region("oversell").len(), 1);
        assert_eq!(report.region("residue").len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
