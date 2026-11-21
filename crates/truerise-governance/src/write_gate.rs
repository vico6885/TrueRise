//! The persistent-state write gate (G29): the shared door for the four
//! persistent-state write paths - memory writes, skill loads, proposal
//! applies, baseline updates. Wiring status: the evolve apply path and the
//! run hat-load path are gated; the Python SDK baseline surface is an
//! explicitly DECLERATED deferral (its contract is frozen pre-release).
//! The in-repo wiring is proven at the library boundaries; the
//! run-path-wide sweep continues with Phase C.
//!
//! Rule (spec L7): any assertion written to persistent state must carry a
//! SUPPORT REFERENCE - a G25 recomputation result, a G26 action-evidence
//! digest, or a G1 external-truth attestation. Without one, the gate
//! either REFUSES the write (memory, skill, baseline) or admits it marked
//! as an `unverified_assumption` (proposal apply - the human-approved
//! override) that then SURVIVES into every later generation certificate:
//! a marked assumption can never quietly become "looks verified".
//!
//! Honest boundaries, both mechanical: (1) washing a mark requires one of
//! the three support kinds, and the wash itself lands on the audit chain -
//! a mark can be paid off, never deleted; the gate records WHO washed with
//! WHAT KIND, while verifying the support PAYLOAD belongs to the
//! G25/G26/G1 machinery (post-hoc audit of wash rows against real
//! artifacts is that machinery's job). (2) Cross-generation survival is
//! MACHINERY: marks are reconstructed from the chain rows at generation
//! start ([`UnverifiedLedger::reconstruct_from_chain`]) and certificate
//! rows are embedded by the evolve certificate - not caller discipline.

use crate::chain::HashChain;
use sha2::{Digest, Sha256};
use truerise_wire::Severity;

/// The four persistent-state write paths that share this gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePath {
    /// A record persisted into the memory backend.
    MemoryWrite,
    /// A skill/hat loaded into the runtime.
    SkillLoad,
    /// A generation proposal applied to persistent skills.
    ProposalApply,
    /// A baseline (ratchet) update persisted to disk.
    BaselineUpdate,
}

impl WritePath {
    /// Canonical token for chain rows and certificate rows.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemoryWrite => "memory_write",
            Self::SkillLoad => "skill_load",
            Self::ProposalApply => "proposal_apply",
            Self::BaselineUpdate => "baseline_update",
        }
    }
}

/// The three legitimate support kinds (spec: G25 / G26 / G1, three-choose-one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportRef {
    /// A G25 closed-form recomputation result: the assertion was re-derived
    /// from committed inputs and the recomputation digest matches.
    Recomputation {
        /// What was recomputed.
        claim: String,
        /// The recomputation digest.
        digest: String,
    },
    /// A G26 action-evidence digest: a signed trajectory step proves the
    /// action happened.
    ActionEvidence {
        /// The signed step's digest.
        step_digest: String,
    },
    /// A G1 external-truth attestation: an authority source (sealed before
    /// the run) carries the ground truth.
    ExternalTruth {
        /// The authority source id.
        source: String,
        /// The attested digest.
        digest: String,
    },
}

impl SupportRef {
    /// The support kind token.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Recomputation { .. } => "g25_recomputation",
            Self::ActionEvidence { .. } => "g26_action_evidence",
            Self::ExternalTruth { .. } => "g1_external_truth",
        }
    }

    /// A stable digest of the support payload (for chain rows).
    #[must_use]
    pub fn payload_digest(&self) -> String {
        let text = match self {
            Self::Recomputation { claim, digest } => format!("recomputation:{claim}:{digest}"),
            Self::ActionEvidence { step_digest } => format!("action:{step_digest}"),
            Self::ExternalTruth { source, digest } => format!("external:{source}:{digest}"),
        };
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let out = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for b in out.iter() {
            hex.push_str(&format!("{b:02x}"));
        }
        hex
    }
}

/// Write-gate refusals. Every variant is a refusal, never a downgrade.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteGateError {
    /// A write that the path's policy refuses (no support reference).
    #[error(
        "write gate refused: {path} requires a support reference (g25/g26/g1), assertion {assertion_digest}"
    )]
    UnsupportedWrite {
        /// The write path that refused.
        path: &'static str,
        /// Digest of the refused assertion.
        assertion_digest: String,
    },
    /// A mark that does not exist or was already washed.
    #[error("write gate refused: assumption {0} unknown or already washed")]
    UnknownOrWashed(String),
    /// Chain events could not be appended (the chain is the single
    /// authority - a gate decision that cannot land on the chain does not
    /// happen).
    #[error("write gate refused: decision could not land on the audit chain: {0}")]
    ChainSealed(String),
}

/// Per-path enforcement policy. Three paths refuse un-cited writes; the
/// human-approved proposal-apply path may admit as a MARKED assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathPolicy {
    Refuse,
    Mark,
}

fn policy_for(path: WritePath) -> PathPolicy {
    match path {
        WritePath::MemoryWrite | WritePath::SkillLoad | WritePath::BaselineUpdate => {
            PathPolicy::Refuse
        }
        WritePath::ProposalApply => PathPolicy::Mark,
    }
}

fn assertion_digest(assertion: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(assertion.as_bytes());
    let out = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in out.iter() {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

/// One marked unverified assumption. Washed entries REMAIN in the ledger
/// with their wash record - transparency survives, the mark never
/// silently disappears. Line digests let a skill precondition cite ONE
/// claim of a multi-line marked assertion (review A3-P2-3: whole-text
/// matching alone made the skill gate degenerate in practice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnverifiedAssumption {
    /// Stable id (digest of path + assertion).
    pub id: String,
    /// The path that carried the write.
    pub path: WritePath,
    /// Digest of the asserted text (whole content).
    pub assertion_digest: String,
    /// Digests of each non-empty LINE of the asserted text.
    pub line_digests: Vec<String>,
    /// The generation that marked it.
    pub marked_at_generation: u64,
    /// The wash record, once a legitimate support reference pays it off.
    pub washed: Option<(String, u64)>,
}

/// The persistent register of marked assumptions. The caller (the
/// generation flow) owns it ACROSS generations - the survival contract is
/// that certificate rows keep carrying every entry, washed or not.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct UnverifiedLedger {
    entries: Vec<UnverifiedAssumption>,
}

impl UnverifiedLedger {
    /// Empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark an assertion as an unverified assumption (idempotent by id:
    /// re-marking the same path+assertion is a no-op returning the id).
    ///
    /// # Errors
    /// [`WriteGateError::ChainSealed`] when the chain event cannot land.
    pub fn mark(
        &mut self,
        path: WritePath,
        assertion: &str,
        generation: u64,
        chain: Option<&HashChain>,
    ) -> Result<String, WriteGateError> {
        let digest = assertion_digest(assertion);
        let id = assertion_digest(&format!("{}:{}", path.as_str(), digest));
        if self.entries.iter().any(|e| e.id == id) {
            return Ok(id);
        }
        let line_digests: Vec<String> = assertion
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(assertion_digest)
            .collect();
        // Review A3-P2-1/B3-P2-5: the chain event lands BEFORE the
        // in-memory mutation - a chain-sealed mark must leave NO ledger
        // state behind (the retry re-derives everything from the chain).
        if let Some(chain) = chain {
            chain
                .append(
                    "write_gate.marked_unverified",
                    Severity::Warn,
                    &serde_json::json!({
                        "id": id,
                        "path": path.as_str(),
                        "assertion_digest": digest,
                        "generation": generation,
                    }),
                    None,
                )
                .map_err(|e| WriteGateError::ChainSealed(e.to_string()))?;
        }
        self.entries.push(UnverifiedAssumption {
            id: id.clone(),
            path,
            assertion_digest: digest,
            line_digests,
            marked_at_generation: generation,
            washed: None,
        });
        Ok(id)
    }

    /// Reconstruct the ledger from the audit chain's mark rows (review
    /// A3-P1-2/B3-P1-1: cross-generation survival must be MACHINERY, not
    /// caller discipline - the chain is the persistence, so a generation
    /// start re-derives every prior mark from the rows and nothing can
    /// silently drop them). Wash rows re-attach their wash records.
    ///
    /// # Errors
    /// [`WriteGateError::ChainSealed`] when the chain cannot be read.
    #[must_use]
    pub fn reconstruct_from_chain(chain: &HashChain) -> Self {
        let mut ledger = Self::new();
        for row in chain.governing_rows() {
            let kind = row.event.kind.as_str();
            if kind != "write_gate.marked_unverified" && kind != "write_gate.washed" {
                continue;
            }
            let detail = &row.event.detail;
            if kind == "write_gate.marked_unverified" {
                let path_token = detail.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let path = match path_token {
                    "memory_write" => WritePath::MemoryWrite,
                    "skill_load" => WritePath::SkillLoad,
                    "proposal_apply" => WritePath::ProposalApply,
                    _ => WritePath::BaselineUpdate,
                };
                let id = detail
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let digest = detail
                    .get("assertion_digest")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let generation = detail
                    .get("generation")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if !ledger.entries.iter().any(|e| e.id == id) {
                    ledger.entries.push(UnverifiedAssumption {
                        id,
                        path,
                        assertion_digest: digest,
                        line_digests: Vec::new(),
                        marked_at_generation: generation,
                        washed: None,
                    });
                }
            } else {
                let id = detail.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let kind_token = detail
                    .get("support_kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let generation = detail
                    .get("generation")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if let Some(entry) = ledger.entries.iter_mut().find(|e| e.id == id) {
                    entry.washed = Some((kind_token, generation));
                }
            }
        }
        ledger
    }

    /// Wash a mark with a legitimate support reference (g25/g26/g1 only -
    /// the wash action itself lands on the chain and the entry STAYS in
    /// the ledger, flagged washed).
    ///
    /// # Errors
    /// [`WriteGateError::UnknownOrWashed`] for unknown ids; [`WriteGateError::ChainSealed`]
    /// when the chain event cannot land.
    pub fn wash(
        &mut self,
        id: &str,
        support: &SupportRef,
        generation: u64,
        chain: Option<&HashChain>,
    ) -> Result<(), WriteGateError> {
        let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) else {
            return Err(WriteGateError::UnknownOrWashed(id.to_string()));
        };
        if entry.washed.is_some() {
            return Err(WriteGateError::UnknownOrWashed(id.to_string()));
        }
        // Review A3-P2-1/B3-P2-5: chain event BEFORE the mutation (a
        // chain-sealed wash must not flip the in-memory state).
        if let Some(chain) = chain {
            chain
                .append(
                    "write_gate.washed",
                    Severity::Warn,
                    &serde_json::json!({
                        "id": id,
                        "support_kind": support.kind(),
                        "support_digest": support.payload_digest(),
                        "generation": generation,
                    }),
                    None,
                )
                .map_err(|e| WriteGateError::ChainSealed(e.to_string()))?;
        }
        entry.washed = Some((support.kind().to_string(), generation));
        Ok(())
    }

    /// Certificate rows: EVERY entry, washed or not - this is the
    /// cross-generation transparency contract. Generation n's certificate
    /// still shows the gen-0 mark.
    #[must_use]
    pub fn certificate_rows(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|e| match &e.washed {
                None => format!(
                    "unverified_assumption:{}:gen{}:{}:{}",
                    e.id,
                    e.marked_at_generation,
                    e.path.as_str(),
                    e.assertion_digest
                ),
                Some((kind, wash_gen)) => format!(
                    "unverified_assumption:{}:gen{}:{}:{} washed:{}@gen{}",
                    e.id,
                    e.marked_at_generation,
                    e.path.as_str(),
                    e.assertion_digest,
                    kind,
                    wash_gen
                ),
            })
            .collect()
    }

    /// All entries (washed included).
    #[must_use]
    pub fn entries(&self) -> &[UnverifiedAssumption] {
        &self.entries
    }
}

/// The shared gate. Pure policy over the path table - the ledger and the
/// chain are passed in by the caller that owns the session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteGate {
    generation: u64,
}

impl WriteGate {
    /// A gate for the given generation (marks record it).
    #[must_use]
    pub fn new(generation: u64) -> Self {
        Self { generation }
    }

    /// Decide one persistent write.
    ///
    /// - With a support reference: [`WriteDecision::Admitted`].
    /// - Without: refuse or mark, per the path policy.
    ///
    /// # Errors
    /// [`WriteGateError::UnsupportedWrite`] when the path refuses.
    pub fn decide(
        &self,
        path: WritePath,
        assertion: &str,
        support: Option<&SupportRef>,
        ledger: &mut UnverifiedLedger,
        chain: Option<&HashChain>,
    ) -> Result<WriteDecision, WriteGateError> {
        let Some(support) = support else {
            return match policy_for(path) {
                PathPolicy::Refuse => Err(WriteGateError::UnsupportedWrite {
                    path: path.as_str(),
                    assertion_digest: assertion_digest(assertion),
                }),
                PathPolicy::Mark => {
                    let id = ledger.mark(path, assertion, self.generation, chain)?;
                    Ok(WriteDecision::AdmittedAsUnverifiedAssumption { assumption_id: id })
                }
            };
        };
        if let Some(chain) = chain {
            chain
                .append(
                    "write_gate.admitted",
                    Severity::Info,
                    &serde_json::json!({
                        "path": path.as_str(),
                        "support_kind": support.kind(),
                        "support_digest": support.payload_digest(),
                        "assertion_digest": assertion_digest(assertion),
                        "generation": self.generation,
                    }),
                    None,
                )
                .map_err(|e| WriteGateError::ChainSealed(e.to_string()))?;
        }
        Ok(WriteDecision::Admitted)
    }
}

/// The gate's verdict over one write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteDecision {
    /// Admitted with a legitimate support reference.
    Admitted,
    /// Admitted as a MARKED unverified assumption (proposal-apply path
    /// only); the id survives into every later generation certificate.
    AdmittedAsUnverifiedAssumption {
        /// The ledger id of the marked assumption.
        assumption_id: String,
    },
}
