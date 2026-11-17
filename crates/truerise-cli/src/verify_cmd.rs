//! G39 CLI wiring: `verify --bundle <bundle.json> --intent <contract.toml>`.
//!
//! The bundle is the self-service acceptance package: on the user's machine,
//! without this repository, it checks one expected observation per
//! machine-checkable acceptance item. Three report buckets, no silence:
//! PASS (checked and matching) / FAIL (checked and mismatching) /
//! NOT-VERIFIED (the observation could not even be executed - routed
//! explicitly, never dropped). Exit 0 only when everything PASSED.

use std::path::Path;

use truerise_wire::{
    BundleError, BundleObservation, IntentContractV1, ObservationKind, VerifyBundleV1,
};

/// One executed observation, sorted into a report bucket.
#[derive(Debug, PartialEq, Eq)]
pub enum ObservationOutcome {
    /// Checked and matching.
    Pass,
    /// Checked and mismatching.
    Fail(String),
    /// Could not be executed at all (missing artifact, unparsable file) -
    /// routed to the not-verified bucket, never silent.
    NotVerified(String),
}

/// The file an observation reads: for `file_exists` the whole target, for
/// `artifact_field` the part before the `#field` suffix.
fn observation_file(observation: &BundleObservation) -> &str {
    match observation.kind {
        ObservationKind::FileExists => &observation.target,
        ObservationKind::ArtifactField => observation.target.split_once('#').map_or(
            &observation.target,
            |(file, _)| file,
        ),
    }
}

/// The dotted field path after the `#` (empty = whole document).
fn observation_field(observation: &BundleObservation) -> String {
    observation
        .target
        .split_once('#')
        .map(|(_, field)| field.to_string())
        .unwrap_or_default()
}

/// Execute one observation against the filesystem (paths as given in the
/// bundle - relative paths resolve against the CALLER's cwd, which is what
/// makes the bundle usable outside this repository).
pub fn run_observation(observation: &BundleObservation) -> ObservationOutcome {
    let file = observation_file(observation);
    let path = Path::new(file);
    match observation.kind {
        ObservationKind::FileExists => {
            if path.is_file() {
                ObservationOutcome::Pass
            } else {
                ObservationOutcome::NotVerified(format!("file does not exist: {file}"))
            }
        }
        ObservationKind::ArtifactField => {
            let Ok(text) = std::fs::read_to_string(path) else {
                return ObservationOutcome::NotVerified(format!("artifact unreadable: {file}"));
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
                return ObservationOutcome::NotVerified(format!("artifact is not JSON: {file}"));
            };
            let dotted = observation_field(observation);
            match field_lookup(&value, &dotted) {
                Some(actual) if actual == observation.expected => ObservationOutcome::Pass,
                Some(actual) => ObservationOutcome::Fail(format!(
                    "field {file}#{dotted} = {actual}, expected {}",
                    observation.expected
                )),
                None => ObservationOutcome::NotVerified(format!(
                    "field not found in {file}: {dotted}"
                )),
            }
        }
    }
}

/// Look up a dotted field path in a JSON value.
fn field_lookup(value: &serde_json::Value, dotted: &str) -> Option<String> {
    let mut current = value;
    for segment in dotted.split('.').filter(|s| !s.is_empty()) {
        current = current.get(segment)?;
    }
    Some(match current {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

/// The report: per-observation buckets + the aggregate verdict.
#[derive(Debug)]
pub struct BundleReport {
    /// (item_id, outcome) in bundle order.
    pub outcomes: Vec<(String, ObservationOutcome)>,
}

impl BundleReport {
    /// Exit code: 0 only when EVERY observation passed (a not-verified item
    /// is not a pass - it is an explicit gap).
    pub fn exit_code(&self) -> i32 {
        i32::from(
            self.outcomes
                .iter()
                .any(|(_, o)| !matches!(o, ObservationOutcome::Pass)),
        )
    }

    /// The printed report: per-item buckets, the aggregate line, and the
    /// explicit not-verified routing note (silence is not coverage).
    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        let (mut pass, mut fail, mut skip) = (0usize, 0usize, 0usize);
        for (id, outcome) in &self.outcomes {
            match outcome {
                ObservationOutcome::Pass => {
                    pass += 1;
                    lines.push(format!("  PASS  {id}"));
                }
                ObservationOutcome::Fail(why) => {
                    fail += 1;
                    lines.push(format!("  FAIL  {id}: {why}"));
                }
                ObservationOutcome::NotVerified(why) => {
                    skip += 1;
                    lines.push(format!("  NOT-VERIFIED  {id}: {why}"));
                }
            }
        }
        lines.push(format!(
            "bundle result: {pass} pass / {fail} fail / {skip} not-verified"
        ));
        if skip > 0 {
            lines.push(
                "  note: not-verified items are routed here explicitly - silence is not coverage"
                    .to_string(),
            );
        }
        lines.join("\n")
    }
}

/// Load + validate the bundle against the sealed contract, then execute it.
/// `Err` = refusal (usage error, exit 2); `Ok(report)` = executed bundle.
pub fn run_bundle(bundle_path: &Path, intent_path: &Path) -> Result<BundleReport, String> {
    let contract_text = std::fs::read_to_string(intent_path)
        .map_err(|e| format!("cannot read intent contract {}: {e}", intent_path.display()))?;
    let mut contract: IntentContractV1 = toml::from_str(&contract_text)
        .map_err(|e| format!("intent contract is not valid v1 TOML: {e}"))?;
    contract
        .validate()
        .map_err(|e| format!("intent contract refused: {e}"))?;
    contract.seal();
    let bundle_text = std::fs::read_to_string(bundle_path)
        .map_err(|e| format!("cannot read bundle {}: {e}", bundle_path.display()))?;
    let bundle: VerifyBundleV1 = serde_json::from_str(&bundle_text)
        .map_err(|e| format!("bundle is not valid v1 JSON: {e}"))?;
    bundle
        .validate_against(&contract)
        .map_err(|e: BundleError| format!("bundle refused: {e}"))?;
    Ok(BundleReport {
        outcomes: bundle
            .observations
            .iter()
            .map(|o| (o.item_id.clone(), run_observation(o)))
            .collect(),
    })
}
