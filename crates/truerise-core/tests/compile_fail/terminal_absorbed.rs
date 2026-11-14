```rust
use truerise_core::state_machine::Machine;

fn main() {
    // A finished machine has NO transition methods: this must not compile.
    let m = Machine::new().run_start().plan().act().observe().verdict_issued().finish();
    m.run_start(); // <- error: no method `run_start` on Machine<Finished>
}
```
