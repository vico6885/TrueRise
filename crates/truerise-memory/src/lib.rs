//! truerise-memory: three-way fusion recall (BM25 + recency + importance),
//! NO vector search (spec §6 memory row), and the MemoryBackend port.
//!
//! Score = w_bm25*bm25 + w_recency*recency + w_importance*importance, weights
//! from config (normalised to sum 1.0). All three channels are normalised to
//! [0,1] before fusing, so the weights are meaningful across channels.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One memory record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Stable id.
    pub id: String,
    /// Content text (BM25 corpus field).
    pub text: String,
    /// Importance in [0,1] (static, writer-assigned).
    pub importance: f32,
    /// Recency in [0,1] (1 = just written; decays externally).
    pub recency: f32,
}

/// Fusion weights (config-driven; normalised on load).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weights {
    /// BM25 channel weight.
    pub bm25: f32,
    /// Recency channel weight.
    pub recency: f32,
    /// Importance channel weight.
    pub importance: f32,
}

impl Weights {
    /// Normalise so the three weights sum to 1 (guard against all-zero).
    pub fn normalised(&self) -> Weights {
        let sum = self.bm25 + self.recency + self.importance;
        if sum <= 0.0 {
            return Weights {
                bm25: 1.0 / 3.0,
                recency: 1.0 / 3.0,
                importance: 1.0 / 3.0,
            };
        }
        Weights {
            bm25: self.bm25 / sum,
            recency: self.recency / sum,
            importance: self.importance / sum,
        }
    }
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            bm25: 0.5,
            recency: 0.3,
            importance: 0.2,
        }
    }
}

/// Memory backend port (spec: four-port rule). Implementations store and
/// return records; the fusion ranker works over ANY backend's snapshot.
pub trait MemoryBackend: Send + Sync {
    /// Persist one record (upsert by id).
    ///
    /// # Errors
    /// Implementation-defined (string-typed at the port boundary).
    fn put(&self, record: &Record) -> Result<(), String>;
    /// Snapshot of all records (the fusion ranker is pure over it).
    ///
    /// # Errors
    /// Implementation-defined.
    fn all(&self) -> Result<Vec<Record>, String>;
}

/// In-memory backend (the reference implementation; tests + demos).
#[derive(Debug, Default)]
pub struct InMemoryBackend {
    records: std::sync::Mutex<HashMap<String, Record>>,
}

impl InMemoryBackend {
    /// Empty backend.
    pub fn new() -> Self {
        Self::default()
    }
}

impl MemoryBackend for InMemoryBackend {
    fn put(&self, record: &Record) -> Result<(), String> {
        self.records
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(record.id.clone(), record.clone());
        Ok(())
    }

    fn all(&self) -> Result<Vec<Record>, String> {
        Ok(self
            .records
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .cloned()
            .collect())
    }
}

/// BM25 over the candidate set (k1=1.2, b=0.75; idf = ln(1 + (N-df+0.5)/(df+0.5))
/// clamped >= 0). Scores normalised to [0,1] by the max within the set.
pub fn bm25_scores(query: &str, records: &[Record]) -> Vec<f32> {
    let q_terms: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if q_terms.is_empty() || records.is_empty() {
        return vec![0.0; records.len()];
    }
    let docs: Vec<Vec<String>> = records
        .iter()
        .map(|r| {
            r.text
                .to_lowercase()
                .split(|c: char| !c.is_alphanumeric())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .collect();
    let n = docs.len() as f32;
    let avgdl = docs.iter().map(|d| d.len()).sum::<usize>() as f32 / n;
    let (k1, b) = (1.2f32, 0.75f32);
    let idf = |term: &str| -> f32 {
        let df = docs.iter().filter(|d| d.iter().any(|t| t == term)).count() as f32;
        let v = ((n - df + 0.5) / (df + 0.5)).ln();
        v.max(0.0)
    };
    let mut raw: Vec<f32> = docs
        .iter()
        .map(|doc| {
            let dl = doc.len() as f32;
            let mut score = 0.0;
            for term in &q_terms {
                let tf = doc.iter().filter(|t| *t == term).count() as f32;
                if tf > 0.0 {
                    score += idf(term) * (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * dl / avgdl));
                }
            }
            score
        })
        .collect();
    let max = raw.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        for s in &mut raw {
            *s /= max;
        }
    }
    raw
}

/// Fusion-ranked records for ``query`` (descending). Ties break by id
/// (deterministic ordering).
pub fn recall(
    backend: &dyn MemoryBackend,
    query: &str,
    weights: &Weights,
) -> Result<Vec<(Record, f32)>, String> {
    let records = backend.all()?;
    let w = weights.normalised();
    let bm = bm25_scores(query, &records);
    let max_rec = records
        .iter()
        .map(|r| r.recency)
        .fold(1.0f32, f32::max)
        .max(1e-6);
    let max_imp = records
        .iter()
        .map(|r| r.importance)
        .fold(1.0f32, f32::max)
        .max(1e-6);
    let mut scored: Vec<(Record, f32)> = records
        .into_iter()
        .zip(bm)
        .map(|(r, b)| {
            let recency = r.recency / max_rec;
            let importance = r.importance / max_imp;
            let score = w.bm25 * b + w.recency * recency + w.importance * importance;
            (r, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.id.cmp(&b.0.id)));
    Ok(scored)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn backend() -> InMemoryBackend {
        let b = InMemoryBackend::new();
        b.put(&Record {
            id: "a".into(),
            text: "the audit chain verifies every row".into(),
            importance: 0.9,
            recency: 0.2,
        })
        .unwrap();
        b.put(&Record {
            id: "b".into(),
            text: "the sandbox fence refuses escapes".into(),
            importance: 0.5,
            recency: 0.9,
        })
        .unwrap();
        b.put(&Record {
            id: "c".into(),
            text: "the state machine absorbs terminal events".into(),
            importance: 0.3,
            recency: 0.5,
        })
        .unwrap();
        b
    }

    #[test]
    fn weights_normalise_to_one() {
        let w = Weights {
            bm25: 1.0,
            recency: 1.0,
            importance: 1.0,
        }
        .normalised();
        assert!((w.bm25 + w.recency + w.importance - 1.0).abs() < 1e-6);
        let zero = Weights {
            bm25: 0.0,
            recency: 0.0,
            importance: 0.0,
        }
        .normalised();
        assert!(
            (zero.bm25 - 1.0 / 3.0).abs() < 1e-6,
            "all-zero falls back to uniform"
        );
    }

    #[test]
    fn bm25_ranks_matching_record_first() {
        let b = backend();
        let scores = bm25_scores("audit chain", &b.all().unwrap());
        let max_idx = scores
            .iter()
            .enumerate()
            .max_by(|x, y| x.1.total_cmp(y.1))
            .unwrap()
            .0;
        assert_eq!(max_idx, 0, "record a matches both query terms");
    }

    #[test]
    fn fusion_respects_weights() {
        let b = backend();
        let w = Weights {
            bm25: 0.1,
            recency: 0.8,
            importance: 0.1,
        };
        let ranked = recall(&b, "audit chain", &w).unwrap();
        assert_eq!(ranked[0].0.id, "b", "recency-heavy: record b (0.9) first");
        let w2 = Weights {
            bm25: 0.9,
            recency: 0.05,
            importance: 0.05,
        };
        let ranked2 = recall(&b, "audit chain", &w2).unwrap();
        assert_eq!(ranked2[0].0.id, "a", "bm25-heavy: record a first");
    }

    #[test]
    fn ties_break_by_id_for_determinism() {
        let b = InMemoryBackend::new();
        b.put(&Record {
            id: "z".into(),
            text: "same text".into(),
            importance: 0.5,
            recency: 0.5,
        })
        .unwrap();
        b.put(&Record {
            id: "y".into(),
            text: "same text".into(),
            importance: 0.5,
            recency: 0.5,
        })
        .unwrap();
        let ranked = recall(&b, "same text", &Weights::default()).unwrap();
        assert_eq!(ranked[0].0.id, "y", "identical scores break by id");
    }

    #[test]
    fn backend_upsert_by_id() {
        let b = InMemoryBackend::new();
        b.put(&Record {
            id: "a".into(),
            text: "v1".into(),
            importance: 0.1,
            recency: 0.1,
        })
        .unwrap();
        b.put(&Record {
            id: "a".into(),
            text: "v2".into(),
            importance: 0.2,
            recency: 0.2,
        })
        .unwrap();
        assert_eq!(b.all().unwrap().len(), 1);
        assert_eq!(b.all().unwrap()[0].text, "v2");
    }
}
