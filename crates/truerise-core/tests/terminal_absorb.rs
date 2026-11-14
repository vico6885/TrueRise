//! §6.7.1 judgement 2: terminal absorption is a COMPILE error.
//!
//! Firing any event on a terminal machine (`Machine<Finished>` /
//! `Machine<Failed>`) must not compile - the typestate layer makes illegal
//! terminal transitions unrepresentable.

#[test]
fn terminal_states_absorb_at_compile_time() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
