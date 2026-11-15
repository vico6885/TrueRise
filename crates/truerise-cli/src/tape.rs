//! The recording/replay router (W14): the mechanism that makes model steps
//! REPLAYABLE without keys, without network, without re-prediction.
//!
//! - Record: every router call is captured (tag, request, model, response)
//!   into a tape file whose HEADER carries the run's trace digest.
//! - Replay: calls are answered FROM the tape, byte-exact; a missing entry
//!   is a fatal error (no silent improvisation). The existing round-pinning
//!   check still applies, so a tape from another model voids the round.
//!
//! Honesty rail (docs/determinism.md): a tape proves "this reading happened
//! at record time and recomputes byte-exact"; it does NOT claim the model's
//! future outputs are deterministic.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sha2::{Digest, Sha256};
use truerise_core::{ModelRouter, RouterError, TurnRequest, TurnResponse};

/// One recorded router call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TapeEntry {
    /// The turn tag (plan/act/observe).
    pub tag: String,
    /// The request payload (verbatim).
    pub request: String,
    /// The model that produced the response.
    pub model: String,
    /// The response payload (verbatim).
    pub response: String,
}

impl TapeEntry {
    fn canonical(&self) -> String {
        serde_json::json!({
            "tag": self.tag, "request": self.request,
            "model": self.model, "response": self.response
        })
        .to_string()
    }

    fn key(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.tag.as_bytes());
        hasher.update([0]);
        hasher.update(self.request.as_bytes());
        hex(&hasher.finalize())
    }
}

fn hex(out: &[u8]) -> String {
    let mut s = String::with_capacity(out.len() * 2);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// The run's trace digest: sha256 over the canonical form of every recorded
/// call, in order. Two runs with the same trace digest executed the SAME
/// model steps (the "identical" in the acceptance test).
#[must_use]
pub fn trace_digest(entries: &[TapeEntry]) -> String {
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.canonical().as_bytes());
        hasher.update([0]);
    }
    hex(&hasher.finalize())
}

/// Read a tape file back (header line + one canonical entry per line).
///
/// # Errors
/// IO / parse errors surface as strings.
pub fn read_tape(path: &Path) -> Result<(serde_json::Value, Vec<TapeEntry>), String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut lines = std::io::BufReader::new(file).lines();
    let header_line = lines
        .next()
        .ok_or("tape: empty")?
        .map_err(|e| e.to_string())?;
    let header: serde_json::Value =
        serde_json::from_str(&header_line).map_err(|e| format!("tape header: {e}"))?;
    let mut entries = Vec::new();
    for line in lines {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let doc: serde_json::Value =
            serde_json::from_str(&line).map_err(|e| format!("tape entry: {e}"))?;
        entries.push(TapeEntry {
            tag: doc["tag"].as_str().unwrap_or_default().into(),
            request: doc["request"].as_str().unwrap_or_default().into(),
            model: doc["model"].as_str().unwrap_or_default().into(),
            response: doc["response"].as_str().unwrap_or_default().into(),
        });
    }
    Ok((header, entries))
}

/// The tape-wrapped router. Record calls the inner router and appends;
/// Replay answers from the tape and NEVER calls the inner router.
pub struct TapeRouter<'a> {
    inner: &'a dyn ModelRouter,
    mode: Mode,
    entries: Mutex<Vec<TapeEntry>>,
}

enum Mode {
    Record(PathBuf),
    /// Consumed-take semantics: each replay call consumes the FIRST unused
    /// matching entry - runs execute the same call sequence in the same
    /// order, so identical requests at different positions replay their own
    /// response (two generations may share a request but not an answer).
    Replay(Mutex<Vec<Option<TapeEntry>>>, PathBuf),
}

impl<'a> TapeRouter<'a> {
    /// Wrap ``inner`` and RECORD every call to ``tape_path``.
    #[must_use]
    pub fn recording(inner: &'a dyn ModelRouter, tape_path: PathBuf) -> Self {
        // CONTINUE an existing tape: a fresh recorder over the same path
        // seeds from what is already on disk, so multiple generations
        // append to ONE tape (the trace digest covers the whole run).
        let entries = if tape_path.exists() {
            read_tape(&tape_path).map(|(_, e)| e).unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            inner,
            mode: Mode::Record(tape_path),
            entries: Mutex::new(entries),
        }
    }

    /// Load ``tape_path`` and REPLAY it (``inner`` is required by the trait
    /// shape but is never called in replay mode).
    ///
    /// # Errors
    /// String-typed: unreadable/malformed tape.
    pub fn replaying(tape_path: PathBuf) -> Result<Self, String> {
        let (_, entries) = read_tape(&tape_path)?;
        Ok(Self {
            inner: &PANIC_ROUTER,
            mode: Mode::Replay(
                Mutex::new(entries.into_iter().map(Some).collect()),
                tape_path,
            ),
            entries: Mutex::new(Vec::new()),
        })
    }

    /// The entries recorded (or replayed) so far.
    #[must_use]
    pub fn recorded(&self) -> Vec<TapeEntry> {
        self.entries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

static PANIC_ROUTER: UnreachableRouter = UnreachableRouter;
struct UnreachableRouter;
impl ModelRouter for UnreachableRouter {
    fn route(&self, _: &TurnRequest) -> Result<TurnResponse, RouterError> {
        Err(RouterError::Fatal(
            "replay mode must never call the inner router".into(),
        ))
    }
}

impl ModelRouter for TapeRouter<'_> {
    fn route(&self, request: &TurnRequest) -> Result<TurnResponse, RouterError> {
        match &self.mode {
            Mode::Record(path) => {
                let response = self.inner.route(request)?;
                let entry = TapeEntry {
                    tag: request.tag.clone(),
                    request: request.payload.clone(),
                    model: response.model.clone(),
                    response: response.payload.clone(),
                };
                let recorded = {
                    let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
                    entries.push(entry.clone());
                    entries.clone()
                };
                write_tape(path, &recorded).map_err(RouterError::Fatal)?;
                Ok(response)
            }
            Mode::Replay(store, path) => {
                let mut slots = store.lock().unwrap_or_else(|p| p.into_inner());
                let index = slots.iter().position(|slot| {
                    slot.as_ref()
                        .is_some_and(|e| e.tag == request.tag && e.request == request.payload)
                });
                let Some(index) = index else {
                    return Err(RouterError::Fatal(format!(
                        "tape {}: no UNUSED entry for tag={}",
                        path.display(),
                        request.tag
                    )));
                };
                let Some(entry) = slots[index].take() else {
                    return Err(RouterError::Fatal("tape: entry vanished mid-take".into()));
                };
                Ok(TurnResponse {
                    model: entry.model.clone(),
                    payload: entry.response.clone(),
                })
            }
        }
    }
}

/// Write the tape: header (router name + trace digest) then one canonical
/// entry per line.
fn write_tape(path: &Path, entries: &[TapeEntry]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut out = String::new();
    let header = serde_json::json!({
        "tape_version": 1,
        "entries": entries.len(),
        "trace_digest": trace_digest(entries)
    });
    out.push_str(&header.to_string());
    out.push('\n');
    for entry in entries {
        out.push_str(&entry.canonical());
        out.push('\n');
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

/// A convenience map for the replay-side "same tape" assertions.
#[must_use]
pub fn tape_entries_as_map(entries: &[TapeEntry]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|e| (e.key(), e.response.clone()))
        .collect()
}
