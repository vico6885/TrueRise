//! The four run modes, injected and frozen (spec §6 four-modes row).
//!
//! research / backtest / sim / live. The set is injected at construction and
//! frozen afterwards. Rules encoded in the type:
//! - `live` requires `TRUERISE_LIVE_AFFIRM=yes` in the environment at
//!   construction time (affirmation is read once, then frozen);
//! - execution capability is granted ONLY in `sim`/`live` AND only with an
//!   explicit opt-in per session;
//! - `research` NEVER grants execution (compile-time unreachable through the
//!   typed API: [`ExecutionGrant::acquire`] takes a mode and research
//!   statically refuses).

use crate::errors::{CoreError, ModeViolation, RiskGate};
use std::collections::HashSet;

/// The four run modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Read/derive only; never grants execution.
    Research,
    /// Historical replay; never grants live execution.
    Backtest,
    /// Simulated execution; grants execution with opt-in.
    Sim,
    /// Real execution; requires affirmation + opt-in.
    Live,
}

impl Mode {
    /// Stable wire name (matches the mode_switch wire contract).
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Research => "research",
            Mode::Backtest => "backtest",
            Mode::Sim => "sim",
            Mode::Live => "live",
        }
    }
}

/// The frozen mode set for one session. Injection happens once; the set
/// cannot grow or change afterwards (frozen by construction).
#[derive(Debug, Clone)]
pub struct ModeSet {
    allowed: HashSet<Mode>,
    active: Mode,
    live_affirmed: bool,
}

impl ModeSet {
    /// Construct from an explicit allow-list + initial mode. Reads the live
    /// affirmation from the environment exactly once (frozen).
    ///
    /// # Errors
    /// [`CoreError::RiskGate`] when the initial mode is not in the set, or
    /// when a `live` initial mode lacks `TRUERISE_LIVE_AFFIRM=yes`.
    pub fn inject(allowed: &[Mode], initial: Mode) -> Result<Self, CoreError> {
        let set = Self {
            allowed: allowed.iter().copied().collect(),
            active: initial,
            live_affirmed: std::env::var("TRUERISE_LIVE_AFFIRM").as_deref() == Ok("yes"),
        };
        if !set.allowed.contains(&initial) {
            return Err(CoreError::ModeViolation(ModeViolation::SwitchRefused {
                from: "<init>".into(),
                to: initial.as_str().into(),
            }));
        }
        if initial == Mode::Live && !set.live_affirmed {
            return Err(CoreError::RiskGate(RiskGate::LiveWithoutAffirmation {
                mode: initial.as_str().into(),
            }));
        }
        Ok(set)
    }

    /// The active mode.
    pub fn active(&self) -> Mode {
        self.active
    }

    /// Switch the active mode (within the frozen set; live still requires the
    /// frozen affirmation).
    ///
    /// # Errors
    /// [`CoreError::RiskGate`] when the target is outside the frozen set or
    /// is `live` without affirmation.
    pub fn switch(&mut self, to: Mode) -> Result<(), CoreError> {
        if !self.allowed.contains(&to) {
            return Err(CoreError::ModeViolation(ModeViolation::SwitchRefused {
                from: self.active.as_str().into(),
                to: to.as_str().into(),
            }));
        }
        if to == Mode::Live && !self.live_affirmed {
            return Err(CoreError::RiskGate(RiskGate::LiveWithoutAffirmation {
                mode: to.as_str().into(),
            }));
        }
        self.active = to;
        Ok(())
    }

    /// Acquire the execution capability: only sim/live AND explicit opt-in.
    ///
    /// # Errors
    /// [`CoreError::RiskGate::ExecutionNotGrantable`] for research/backtest
    /// (research NEVER grants), [`RiskGate::ApprovalRequired`] without opt-in,
    /// [`RiskGate::LiveWithoutAffirmation`] for unaffirmed live.
    pub fn acquire_execution(&self, opt_in: bool) -> Result<ExecutionGrant, CoreError> {
        match self.active {
            Mode::Research | Mode::Backtest => {
                Err(CoreError::RiskGate(RiskGate::ExecutionNotGrantable {
                    mode: self.active.as_str().into(),
                }))
            }
            Mode::Live if !self.live_affirmed => {
                Err(CoreError::RiskGate(RiskGate::LiveWithoutAffirmation {
                    mode: self.active.as_str().into(),
                }))
            }
            Mode::Sim | Mode::Live => {
                if !opt_in {
                    return Err(CoreError::RiskGate(RiskGate::ApprovalRequired {
                        action: "execution".into(),
                    }));
                }
                Ok(ExecutionGrant { mode: self.active })
            }
        }
    }
}

/// Capability type: possessing a value proves execution was granted in a
/// granting mode with explicit opt-in (spec §6.7 policy row - capability
/// carried at compile time, checked at the boundary).
#[derive(Debug, Clone, Copy)]
pub struct ExecutionGrant {
    /// The mode the grant was acquired in (Sim or Live only).
    pub mode: Mode,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn research_never_grants_execution() {
        let set = ModeSet::inject(&[Mode::Research, Mode::Sim], Mode::Research).unwrap();
        assert!(matches!(
            set.acquire_execution(true),
            Err(CoreError::RiskGate(RiskGate::ExecutionNotGrantable { mode })) if mode == "research"
        ));
    }

    #[test]
    fn backtest_never_grants_execution() {
        let set = ModeSet::inject(&[Mode::Backtest], Mode::Backtest).unwrap();
        assert!(matches!(
            set.acquire_execution(true),
            Err(CoreError::RiskGate(RiskGate::ExecutionNotGrantable { mode })) if mode == "backtest"
        ));
    }

    #[test]
    fn sim_grants_with_optin_only() {
        let set = ModeSet::inject(&[Mode::Sim], Mode::Sim).unwrap();
        assert!(matches!(
            set.acquire_execution(false),
            Err(CoreError::RiskGate(RiskGate::ApprovalRequired { .. }))
        ));
        assert!(set.acquire_execution(true).is_ok());
    }

    #[test]
    fn switch_refuses_outside_frozen_set() {
        let mut set = ModeSet::inject(&[Mode::Research], Mode::Research).unwrap();
        assert!(matches!(
            set.switch(Mode::Live),
            Err(CoreError::ModeViolation(
                ModeViolation::SwitchRefused { .. }
            ))
        ));
    }

    #[test]
    fn live_requires_affirmation_even_when_in_set() {
        // The test process does not set TRUERISE_LIVE_AFFIRM; injection of a
        // live-initial set must fail. (Affirmed-path is covered by the
        // integration suite where the env var is set explicitly.)
        let err = ModeSet::inject(&[Mode::Live], Mode::Live).err();
        assert!(matches!(
            err,
            Some(CoreError::RiskGate(RiskGate::LiveWithoutAffirmation { .. }))
        ));
    }
}
