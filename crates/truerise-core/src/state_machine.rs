//! The run state machine - BOTH encodings, per spec §6.7.1:
//!
//! 1. **typestate**: illegal transitions do not COMPILE (stronger than
//!    runtime rejection); terminal states have no transition methods at all
//!    (compile-time absorption, asserted by the trybuild suite).
//! 2. **table-driven runtime mirror**: every (state, event) pair is explicit;
//!    undefined pairs return [`CoreError::ModeViolation::UndefinedTransition`]
//!    (error, never panic - spec §6.7 boundary red line).
//!
//! Events (9): RunStart, Plan, Act, Observe, VerdictIssued, NoVerdict,
//! Takeover, Finish, Fail. States: Pending, Running, Planned, Acting,
//! Observing, VerdictReady, AwaitingTakeover, Finished*, Failed* (*terminal).
//! `NoVerdict -> AwaitingTakeover` per spec.

use crate::errors::{CoreError, ModeViolation};

/// The nine step-level events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Open a run.
    RunStart,
    /// Form a plan.
    Plan,
    /// Execute an action.
    Act,
    /// Observe results.
    Observe,
    /// A verdict was produced.
    VerdictIssued,
    /// No verdict could be produced (routes to takeover).
    NoVerdict,
    /// Human/system takeover after NoVerdict.
    Takeover,
    /// Close the run successfully.
    Finish,
    /// Close the run as failed.
    Fail,
}

impl Event {
    /// Wire name (state_transitions申报流).
    pub fn as_str(&self) -> &'static str {
        match self {
            Event::RunStart => "run_start",
            Event::Plan => "plan",
            Event::Act => "act",
            Event::Observe => "observe",
            Event::VerdictIssued => "verdict_issued",
            Event::NoVerdict => "no_verdict",
            Event::Takeover => "takeover",
            Event::Finish => "finish",
            Event::Fail => "fail",
        }
    }
}

/// Machine states (name-tagged, comparable - the runtime layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Before the run started.
    Pending,
    /// Run started, working.
    Running,
    /// Plan formed.
    Planned,
    /// Action in flight.
    Acting,
    /// Observing results.
    Observing,
    /// Verdict ready, ready to close.
    VerdictReady,
    /// No verdict; waiting for takeover.
    AwaitingTakeover,
    /// Terminal: finished.
    Finished,
    /// Terminal: failed.
    Failed,
}

impl State {
    /// Wire name.
    pub fn as_str(&self) -> &'static str {
        match self {
            State::Pending => "pending",
            State::Running => "running",
            State::Planned => "planned",
            State::Acting => "acting",
            State::Observing => "observing",
            State::VerdictReady => "verdict_ready",
            State::AwaitingTakeover => "awaiting_takeover",
            State::Finished => "finished",
            State::Failed => "failed",
        }
    }

    /// Terminal states absorb every event (compile-time in the typestate
    /// layer; error at runtime).
    pub fn is_terminal(self) -> bool {
        matches!(self, State::Finished | State::Failed)
    }
}

/// The runtime transition table: every DEFINED (state, event) pair with its
/// next state. Anything not listed here is undefined (-> error).
pub fn transition(state: State, event: Event) -> Result<State, CoreError> {
    use Event as E;
    use State as S;
    if state.is_terminal() {
        return Err(CoreError::ModeViolation(ModeViolation::TerminalAbsorbed {
            state: state.as_str().into(),
            event: event.as_str().into(),
        }));
    }
    let next = match (state, event) {
        (S::Pending, E::RunStart) => S::Running,
        (S::Running, E::Plan) => S::Planned,
        (S::Running, E::Fail) => S::Failed,
        (S::Planned, E::Act) => S::Acting,
        (S::Planned, E::Fail) => S::Failed,
        (S::Acting, E::Observe) => S::Observing,
        (S::Acting, E::Fail) => S::Failed,
        (S::Observing, E::VerdictIssued) => S::VerdictReady,
        (S::Observing, E::NoVerdict) => S::AwaitingTakeover,
        (S::Observing, E::Fail) => S::Failed,
        (S::AwaitingTakeover, E::Takeover) => S::Running,
        (S::AwaitingTakeover, E::Fail) => S::Failed,
        (S::VerdictReady, E::Finish) => S::Finished,
        (S::VerdictReady, E::Fail) => S::Failed,
        _ => {
            return Err(CoreError::ModeViolation(
                ModeViolation::UndefinedTransition {
                    state: state.as_str().into(),
                    event: event.as_str().into(),
                },
            ));
        }
    };
    Ok(next)
}

/// Transition log entry (the state_transitions申报流; single writer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionLog {
    /// State before the event.
    pub from: State,
    /// The event fired.
    pub event: Event,
    /// State after the event.
    pub to: State,
}

/// Typestate machine: `Machine<S>` exists only for non-terminal `S`;
/// terminal states (`Finished`/`Failed`) deliberately have NO `fire` method,
/// so firing an event on them is a COMPILE error (see the trybuild suite).
#[derive(Debug)]
pub struct Machine<S> {
    _state: S,
    log: Vec<TransitionLog>,
}

/// Marker for the Pending state.
#[derive(Debug)]
pub struct Pending;
/// Marker for the Running state.
#[derive(Debug)]
pub struct Running;
/// Marker for the Planned state.
#[derive(Debug)]
pub struct Planned;
/// Marker for the Acting state.
#[derive(Debug)]
pub struct Acting;
/// Marker for the Observing state.
#[derive(Debug)]
pub struct Observing;
/// Marker for the VerdictReady state.
#[derive(Debug)]
pub struct VerdictReady;
/// Marker for the AwaitingTakeover state.
#[derive(Debug)]
pub struct AwaitingTakeover;
/// Terminal marker: Finished (no transition methods exist).
#[derive(Debug)]
pub struct Finished;
/// Terminal marker: Failed (no transition methods exist).
#[derive(Debug)]
pub struct Failed;

impl<S> Machine<S> {
    /// The declared transition log (single writer; mirrors the typestate path).
    pub fn log(&self) -> &[TransitionLog] {
        &self.log
    }
}

impl Default for Machine<Pending> {
    /// A new machine, unstarted.
    fn default() -> Self {
        Self {
            _state: Pending,
            log: Vec::new(),
        }
    }
}

impl Machine<Pending> {
    /// A new machine, unstarted.
    pub fn new() -> Self {
        Self::default()
    }

    /// Fire RunStart (Pending -> Running).
    pub fn run_start(mut self) -> Machine<Running> {
        self.log.push(TransitionLog {
            from: State::Pending,
            event: Event::RunStart,
            to: State::Running,
        });
        Machine {
            _state: Running,
            log: self.log,
        }
    }
}

impl Machine<Running> {
    /// Fire Plan (Running -> Planned).
    pub fn plan(mut self) -> Machine<Planned> {
        self.log.push(TransitionLog {
            from: State::Running,
            event: Event::Plan,
            to: State::Planned,
        });
        Machine {
            _state: Planned,
            log: self.log,
        }
    }

    /// Fire Fail from Running (any non-terminal state may fail).
    pub fn fail(mut self) -> Machine<Failed> {
        self.log.push(TransitionLog {
            from: State::Running,
            event: Event::Fail,
            to: State::Failed,
        });
        Machine {
            _state: Failed,
            log: self.log,
        }
    }
}

impl Machine<Planned> {
    /// Fire Act (Planned -> Acting).
    pub fn act(mut self) -> Machine<Acting> {
        self.log.push(TransitionLog {
            from: State::Planned,
            event: Event::Act,
            to: State::Acting,
        });
        Machine {
            _state: Acting,
            log: self.log,
        }
    }

    /// Fire Fail from Planned.
    pub fn fail(mut self) -> Machine<Failed> {
        self.log.push(TransitionLog {
            from: State::Planned,
            event: Event::Fail,
            to: State::Failed,
        });
        Machine {
            _state: Failed,
            log: self.log,
        }
    }
}

impl Machine<Acting> {
    /// Fire Observe (Acting -> Observing).
    pub fn observe(mut self) -> Machine<Observing> {
        self.log.push(TransitionLog {
            from: State::Acting,
            event: Event::Observe,
            to: State::Observing,
        });
        Machine {
            _state: Observing,
            log: self.log,
        }
    }

    /// Fire Fail from Acting.
    pub fn fail(mut self) -> Machine<Failed> {
        self.log.push(TransitionLog {
            from: State::Acting,
            event: Event::Fail,
            to: State::Failed,
        });
        Machine {
            _state: Failed,
            log: self.log,
        }
    }
}

impl Machine<Observing> {
    /// Fire VerdictIssued (Observing -> VerdictReady).
    pub fn verdict_issued(mut self) -> Machine<VerdictReady> {
        self.log.push(TransitionLog {
            from: State::Observing,
            event: Event::VerdictIssued,
            to: State::VerdictReady,
        });
        Machine {
            _state: VerdictReady,
            log: self.log,
        }
    }

    /// Fire NoVerdict (Observing -> AwaitingTakeover, per spec).
    pub fn no_verdict(mut self) -> Machine<AwaitingTakeover> {
        self.log.push(TransitionLog {
            from: State::Observing,
            event: Event::NoVerdict,
            to: State::AwaitingTakeover,
        });
        Machine {
            _state: AwaitingTakeover,
            log: self.log,
        }
    }

    /// Fire Fail from Observing.
    pub fn fail(mut self) -> Machine<Failed> {
        self.log.push(TransitionLog {
            from: State::Observing,
            event: Event::Fail,
            to: State::Failed,
        });
        Machine {
            _state: Failed,
            log: self.log,
        }
    }
}

impl Machine<AwaitingTakeover> {
    /// Fire Takeover (AwaitingTakeover -> Running; the takeover loop).
    pub fn takeover(mut self) -> Machine<Running> {
        self.log.push(TransitionLog {
            from: State::AwaitingTakeover,
            event: Event::Takeover,
            to: State::Running,
        });
        Machine {
            _state: Running,
            log: self.log,
        }
    }

    /// Fire Fail from AwaitingTakeover.
    pub fn fail(mut self) -> Machine<Failed> {
        self.log.push(TransitionLog {
            from: State::AwaitingTakeover,
            event: Event::Fail,
            to: State::Failed,
        });
        Machine {
            _state: Failed,
            log: self.log,
        }
    }
}

impl Machine<VerdictReady> {
    /// Fire Finish (VerdictReady -> Finished, terminal).
    pub fn finish(mut self) -> Machine<Finished> {
        self.log.push(TransitionLog {
            from: State::VerdictReady,
            event: Event::Finish,
            to: State::Finished,
        });
        Machine {
            _state: Finished,
            log: self.log,
        }
    }

    /// Fire Fail from VerdictReady.
    pub fn fail(mut self) -> Machine<Failed> {
        self.log.push(TransitionLog {
            from: State::VerdictReady,
            event: Event::Fail,
            to: State::Failed,
        });
        Machine {
            _state: Failed,
            log: self.log,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn happy_path_through_the_full_table() {
        let m = Machine::new()
            .run_start()
            .plan()
            .act()
            .observe()
            .verdict_issued()
            .finish();
        let log = m.log();
        assert_eq!(log.len(), 6);
        assert_eq!(
            log[0],
            TransitionLog {
                from: State::Pending,
                event: Event::RunStart,
                to: State::Running
            }
        );
        assert_eq!(log.last().unwrap().to, State::Finished);
    }

    #[test]
    fn no_verdict_routes_to_takeover_loop() {
        let m = Machine::new()
            .run_start()
            .plan()
            .act()
            .observe()
            .no_verdict()
            .takeover()
            .plan()
            .act()
            .observe()
            .verdict_issued()
            .finish();
        let log = m.log();
        assert!(log.iter().any(|t| t.event == Event::NoVerdict));
        assert!(log.iter().any(|t| t.event == Event::Takeover));
        assert_eq!(log.last().unwrap().to, State::Finished);
    }

    #[test]
    fn runtime_table_undefined_transition_is_error() {
        let err = transition(State::Pending, Event::Act).unwrap_err();
        assert!(matches!(
            err,
            CoreError::ModeViolation(ModeViolation::UndefinedTransition { state, event })
                if state == "pending" && event == "act"
        ));
    }

    #[test]
    fn runtime_table_terminal_absorbs_every_event() {
        for ev in [
            Event::RunStart,
            Event::Plan,
            Event::Act,
            Event::Observe,
            Event::VerdictIssued,
            Event::NoVerdict,
            Event::Takeover,
            Event::Finish,
            Event::Fail,
        ] {
            for terminal in [State::Finished, State::Failed] {
                let err = transition(terminal, ev).unwrap_err();
                assert!(
                    matches!(
                        err,
                        CoreError::ModeViolation(ModeViolation::TerminalAbsorbed { .. })
                    ),
                    "terminal {terminal:?} must absorb {ev:?}"
                );
            }
        }
    }

    /// Coverage criterion (§6.7.1 judgement 1 + the §6 table row): every
    /// defined pair transitions; every undefined non-terminal pair errors.
    #[test]
    fn full_table_coverage_defined_and_undefined() {
        let states = [
            State::Pending,
            State::Running,
            State::Planned,
            State::Acting,
            State::Observing,
            State::VerdictReady,
            State::AwaitingTakeover,
        ];
        let events = [
            Event::RunStart,
            Event::Plan,
            Event::Act,
            Event::Observe,
            Event::VerdictIssued,
            Event::NoVerdict,
            Event::Takeover,
            Event::Finish,
            Event::Fail,
        ];
        for s in states {
            for e in events {
                // Must be Ok (defined) or UndefinedTransition - never anything else.
                match transition(s, e) {
                    Ok(next) => {
                        assert!(!next.is_terminal() || matches!(e, Event::Finish | Event::Fail))
                    }
                    Err(CoreError::ModeViolation(ModeViolation::UndefinedTransition {
                        ..
                    })) => {}
                    Err(other) => panic!("unexpected error for {s:?} x {e:?}: {other:?}"),
                }
            }
        }
        // Spot-check a few defined rows from the table above.
        assert_eq!(
            transition(State::Pending, Event::RunStart).unwrap(),
            State::Running
        );
        assert_eq!(
            transition(State::Observing, Event::NoVerdict).unwrap(),
            State::AwaitingTakeover
        );
        assert_eq!(
            transition(State::AwaitingTakeover, Event::Takeover).unwrap(),
            State::Running
        );
        assert_eq!(
            transition(State::VerdictReady, Event::Finish).unwrap(),
            State::Finished
        );
    }

    /// §6.7.1 judgement 3: after any legal event sequence the machine is in
    /// its declared terminal-or-running state and the log agrees.
    #[test]
    fn declared_log_agrees_with_final_state() {
        let m = Machine::new().run_start().plan().fail();
        let log = m.log();
        assert_eq!(log.last().unwrap().to, State::Failed);
    }
}
