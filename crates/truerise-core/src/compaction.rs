//! Five-level compaction (spec §6 compaction row, both duplicate spec lines
//! merged): after compaction the token count moves monotonically DOWN, a
//! pre_compact event is emitted (>= 1), and the most recent N entries are
//! kept VERBATIM (full fidelity tail).
//!
//! Levels: L0 raw -> L1 whitespace-normalise -> L2 per-entry fold ->
//! L3 rolling summary -> L4 skeleton. Each level is strictly <= the previous.

/// One transcript entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Who produced it (user/model/system).
    pub role: String,
    /// Content.
    pub text: String,
    /// Integer token estimate (spec: integer units).
    pub tokens: u64,
}

/// The compaction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Raw (no compaction).
    L0,
    /// Whitespace normalisation.
    L1,
    /// Fold each entry to its first sentence.
    L2,
    /// Fold to a rolling summary line per entry.
    L3,
    /// Skeleton: role tag + token count only.
    L4,
}

/// A pre_compact event (>= 1 per compaction, spec invariant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreCompactEvent {
    /// Level applied.
    pub level: Level,
    /// Tokens before.
    pub tokens_before: u64,
    /// Tokens after.
    pub tokens_after: u64,
    /// Entries kept verbatim at the tail.
    pub tail_kept: usize,
}

/// Errors from compaction.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompactionError {
    /// The most-recent-N tail would be dropped (spec invariant).
    #[error("compaction: tail window ({0}) exceeds history length")]
    TailTooLarge(usize),
    /// An unknown level was requested.
    #[error("compaction: unknown level {0}")]
    UnknownLevel(u32),
}

/// Total tokens of a history.
pub fn total_tokens(entries: &[Entry]) -> u64 {
    entries.iter().map(|e| e.tokens).sum()
}

fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fold_first_sentence(text: &str) -> String {
    let n = normalise(text);
    n.split(['。', '.', '！', '!', '？', '?'])
        .next()
        .unwrap_or(&n)
        .to_string()
}

fn estimate_tokens(text: &str) -> u64 {
    // Integer estimate: words + punctuation clusters. Deterministic, cheap.
    (text.split_whitespace().count() + text.len() / 8) as u64
}

/// Compact ``history`` to ``level``, keeping the most recent ``tail_keep``
/// entries verbatim. Emits exactly one pre_compact event.
///
/// # Errors
/// [`CompactionError::TailTooLarge`] when the tail window exceeds history.
pub fn compact(
    history: &[Entry],
    level: Level,
    tail_keep: usize,
) -> Result<(Vec<Entry>, PreCompactEvent), CompactionError> {
    if tail_keep > history.len() {
        return Err(CompactionError::TailTooLarge(tail_keep));
    }
    let before = total_tokens(history);
    let split_at = history.len() - tail_keep;
    let (head, tail) = history.split_at(split_at);
    let mut out: Vec<Entry> = Vec::with_capacity(history.len());
    for e in head {
        let text = match level {
            Level::L0 => e.text.clone(),
            Level::L1 => normalise(&e.text),
            Level::L2 => fold_first_sentence(&e.text),
            Level::L3 => format!(
                "[{}] {}",
                e.role,
                normalise(&e.text).chars().take(60).collect::<String>()
            ),
            Level::L4 => format!("[{}]", e.role),
        };
        // Compaction NEVER inflates: clamp to the original entry's tokens.
        let tokens = estimate_tokens(&text).min(e.tokens);
        out.push(Entry {
            role: e.role.clone(),
            text,
            tokens,
        });
    }
    out.extend_from_slice(tail);
    let after = total_tokens(&out);
    // Invariant: monotonic non-increase.
    debug_assert!(after <= before, "compaction must not inflate tokens");
    Ok((
        out,
        PreCompactEvent {
            level,
            tokens_before: before,
            tokens_after: after,
            tail_kept: tail_keep,
        },
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn sample() -> Vec<Entry> {
        vec![
            Entry {
                role: "user".into(),
                text: "Please  analyse   the window. Then report!".into(),
                tokens: 20,
            },
            Entry {
                role: "model".into(),
                text: "The  window  is  clean. No leaks found.".into(),
                tokens: 25,
            },
            Entry {
                role: "system".into(),
                text: "Chain verified. All  gates  green.".into(),
                tokens: 15,
            },
            Entry {
                role: "model".into(),
                text: "Final answer: PASS.".into(),
                tokens: 10,
            },
        ]
    }

    /// Spec invariant 1: token count moves monotonically DOWN at every level.
    #[test]
    fn tokens_monotonically_decrease_across_levels() {
        let history = sample();
        // Chained compaction (the real workflow: L1 output feeds L2, ...):
        // tokens must never increase as levels deepen.
        let mut current = history.clone();
        let mut prev = total_tokens(&history);
        for level in [Level::L1, Level::L2, Level::L3, Level::L4] {
            let (out, _ev) = compact(&current, level, 1).unwrap();
            let t = total_tokens(&out);
            assert!(t <= prev, "level {level:?}: {t} > {prev}");
            current = out;
            prev = t;
        }
    }

    /// Spec invariant 2: at least one pre_compact event per compaction.
    #[test]
    fn pre_compact_event_emitted() {
        let (out, ev) = compact(&sample(), Level::L2, 1).unwrap();
        assert_eq!(ev.level, Level::L2);
        assert_eq!(ev.tokens_before, total_tokens(&sample()));
        assert_eq!(ev.tokens_after, total_tokens(&out));
    }

    /// Spec invariant 3: the most recent N entries stay verbatim.
    #[test]
    fn recent_tail_kept_verbatim() {
        let history = sample();
        let (out, ev) = compact(&history, Level::L4, 2).unwrap();
        assert_eq!(ev.tail_kept, 2);
        assert_eq!(
            out[out.len() - 2..],
            history[history.len() - 2..],
            "tail verbatim"
        );
        // Compacted head is a skeleton:
        assert_eq!(out[0].text, "[user]");
    }

    #[test]
    fn tail_too_large_refused() {
        assert!(matches!(
            compact(&sample(), Level::L1, 10),
            Err(CompactionError::TailTooLarge(10))
        ));
    }

    #[test]
    fn unknown_level_refused() {
        // Level is an enum; the u32 parse path (wire) must refuse unknowns.
        let raw: u32 = 9;
        assert!(matches!(
            Level::try_from(raw),
            Err(CompactionError::UnknownLevel(9))
        ));
    }

    impl TryFrom<u32> for Level {
        type Error = CompactionError;
        fn try_from(v: u32) -> Result<Self, Self::Error> {
            match v {
                0 => Ok(Level::L0),
                1 => Ok(Level::L1),
                2 => Ok(Level::L2),
                3 => Ok(Level::L3),
                4 => Ok(Level::L4),
                other => Err(CompactionError::UnknownLevel(other)),
            }
        }
    }
}
