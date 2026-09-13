// Engine v2 header — unrelated lines inserted above the function.
// The engine changelog: v2 adds persistence flags and tracing.
// (no symbols are added or moved by this header)
// see docs for the full v2 changelog

pub fn tick(state: &State) -> u32 {
    reset_state(state);
    step_state(state);
    check_state(state);
    commit_state(state);
    0
}

pub fn startup_hook(state: &State) -> u32 {
    boot_state(state);
    0
}
