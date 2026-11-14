//! Process surface: waiting, killing, identity. The only process boundary.

use std::io;
use std::process::{Child, Command};
use std::time::Duration;

/// Extension over [`Child`] with integer-millisecond waiting (spec: integer
/// units for metering; the std Duration conversion happens here and only here).
pub trait ChildWait {
    /// Wait with a timeout; `Ok(Some(status))` = exited, `Ok(None)` = still
    /// running after ``timeout_ms``. The child is NOT killed on timeout.
    fn wait_ms(&mut self, timeout_ms: u64) -> io::Result<Option<std::process::ExitStatus>>;
}

impl ChildWait for Child {
    fn wait_ms(&mut self, timeout_ms: u64) -> io::Result<Option<std::process::ExitStatus>> {
        // Poll loop on try_wait - the portable wait-with-timeout primitive.
        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            match self.try_wait()? {
                Some(status) => return Ok(Some(status)),
                None if std::time::Instant::now() >= deadline => return Ok(None),
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }
}

/// Spawn a child process (the harness's only spawn point; governance layers
/// wrap this - never spawn around it).
pub fn spawn(program: &str, args: &[&str]) -> io::Result<Child> {
    Command::new(program).args(args).spawn()
}

/// Current process id (identity for lease ownership records).
pub fn current_pid() -> u32 {
    std::process::id()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn wait_ms_returns_none_for_sleeping_child() {
        #[cfg(windows)]
        let mut child = spawn("cmd", &["/C", "ping", "-n", "5", "127.0.0.1"]).unwrap();
        #[cfg(not(windows))]
        let mut child = spawn("sleep", &["1"]).unwrap();
        let r = child.wait_ms(50).unwrap();
        assert!(r.is_none(), "child should still be running after 50ms");
        let _ = child.kill();
        let _ = child.wait();
    }

    #[test]
    fn wait_ms_collects_fast_exit() {
        #[cfg(windows)]
        let mut child = spawn("cmd", &["/C", "exit", "0"]).unwrap();
        #[cfg(not(windows))]
        let mut child = spawn("true", &[]).unwrap();
        let r = child.wait_ms(5_000).unwrap();
        assert!(r.is_some(), "fast child must be reaped within the timeout");
    }

    #[test]
    fn current_pid_is_nonzero() {
        assert!(current_pid() != 0);
    }
}
