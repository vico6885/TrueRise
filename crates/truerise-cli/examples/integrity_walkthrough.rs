//! Embedding the integrity walkthrough in your own code: call the library
//! function directly (no CLI), derive the verdict, and flip it with one
//! honest input. Run with:
//!
//! ```bash
//! cargo run -p truerise-cli --example integrity_walkthrough
//! ```

use truerise_cli::demo_scenarios;

fn main() {
    for honest in [false, true] {
        let (reasons, seal) = demo_scenarios(honest);
        let verdict = if reasons.is_empty() { "PASS" } else { "FAIL" };
        println!(
            "honest={honest} verdict={verdict} caught={} seal={}",
            reasons.len(),
            &seal[..12]
        );
        for reason in &reasons {
            println!("  {reason}");
        }
    }
    println!("-- the same code path, flipped by one input; embed truerise-cli as a library");
}
