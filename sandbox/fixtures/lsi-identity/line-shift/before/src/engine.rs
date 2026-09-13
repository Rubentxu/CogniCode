pub fn tick(state: &State) -> u32 {
    reset_state(state);
    step_state(state);
    check_state(state);
    commit_state(state);
    0
}

pub fn retired_hook(state: &State) -> u32 {
    trace_hook(state);
    0
}
