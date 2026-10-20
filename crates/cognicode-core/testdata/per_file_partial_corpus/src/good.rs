// Fixture: clean Rust file, used to confirm the strategy picks up at
// least one symbol from the corpus. The function `good_fn` is the
// oracle anchor: any correct implementation must include it in the
// resulting graph.

pub fn good_fn() -> u32 {
    42
}
