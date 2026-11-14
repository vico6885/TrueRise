//! Monotonic time source for leases, timeouts and duration accounting.
//!
//! Spec (§6 platform row): lease/timeout logic MUST use a monotonic clock;
//! wall-clock is forbidden there. All durations in this crate are integer
//! milliseconds (spec: integer units for ledger/metering dimensions - no
//! float seconds anywhere).

use std::time::Instant;

/// A monotonic instant captured from [`MonotonicClock`].
///
/// Deliberately opaque: there is no way to read a wall-clock value out of it,
/// so lease/timeout code cannot accidentally mix the two time domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Monotonic(Instant);

/// Integer-milliseconds duration for lease/timeout accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Millis(pub u64);

impl Millis {
    /// Zero duration.
    pub const ZERO: Millis = Millis(0);
}

/// The only clock the harness may use for ordering and timeouts.
#[derive(Debug, Clone, Copy, Default)]
pub struct MonotonicClock {
    _private: (),
}

impl MonotonicClock {
    /// Capture a monotonic instant.
    pub fn now(&self) -> Monotonic {
        Monotonic(Instant::now())
    }

    /// Elapsed integer milliseconds since ``since`` (monotonic domain only).
    pub fn elapsed_ms(&self, since: Monotonic) -> Millis {
        Millis(since.0.elapsed().as_millis() as u64)
    }

    /// True when ``since`` is at least ``timeout`` old (monotonic domain only).
    pub fn expired(&self, since: Monotonic, timeout: Millis) -> bool {
        self.elapsed_ms(since).0 >= timeout.0
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::thread::sleep;

    #[test]
    fn monotonic_ordering_holds() {
        let clock = MonotonicClock::default();
        let a = clock.now();
        let b = clock.now();
        assert!(b >= a, "monotonic instants never go backwards");
    }

    #[test]
    fn elapsed_is_monotonic_and_positive_after_sleep() {
        let clock = MonotonicClock::default();
        let start = clock.now();
        sleep(std::time::Duration::from_millis(15));
        let ms = clock.elapsed_ms(start).0;
        assert!(ms >= 10, "elapsed should reflect the sleep, got {ms}ms");
        sleep(std::time::Duration::from_millis(5));
        assert!(clock.elapsed_ms(start).0 >= ms, "monotonic: never shrinks");
    }

    #[test]
    fn expiry_uses_monotonic_domain() {
        let clock = MonotonicClock::default();
        let start = clock.now();
        assert!(!clock.expired(start, Millis(10_000)));
        assert!(clock.expired(start, Millis::ZERO));
    }
}
