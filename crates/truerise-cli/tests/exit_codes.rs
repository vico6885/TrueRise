//! Exit-code contract (2A): the CLI's exit code must carry the verdict, the
//! same way the Python CLI does (FAIL -> 1). A verdict that cannot turn a CI
//! lane red is a note, not a gate — and this repository sells gates.
//!
//! Relaxed for the unknown-command case only: `cli_main` prints help there,
//! no unwraps are involved.

use truerise_cli::cli_main;

#[test]
fn demo_fail_verdict_exits_nonzero() {
    assert_eq!(
        cli_main(vec!["truerise".into(), "demo".into()]),
        1,
        "a FAIL verdict must exit 1 so CI can gate on it"
    );
}

#[test]
fn demo_honest_verdict_exits_zero() {
    assert_eq!(
        cli_main(vec!["truerise".into(), "demo".into(), "--honest".into()]),
        0,
        "flipping the inputs to honest must clear the exit code"
    );
}

#[test]
fn exit_code_tracks_the_verdict_not_the_flag() {
    // Both calls run the same code path; only the inputs differ. If the exit
    // code ever becomes constant, one of these two assertions breaks.
    let cheating = cli_main(vec!["truerise".into(), "demo".into()]);
    let honest = cli_main(vec!["truerise".into(), "demo".into(), "--honest".into()]);
    assert_ne!(
        cheating, honest,
        "the two verdicts must not share an exit code"
    );
    assert!(cheating > honest, "FAIL must be the non-zero branch");
}

#[test]
fn unknown_command_exits_two() {
    assert_eq!(cli_main(vec!["truerise".into(), "not-a-command".into()]), 2);
}
