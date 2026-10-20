// nested/mod.rs — defines `callee_in_nested`, plus homonyms that test the resolver.

pub fn callee_in_nested() {
    // intentionally empty — single-candidate cross-file target.
}

pub fn local_helper() {
    // intentionally empty — HOMONYM of src/lib.rs::local_helper.
    // Caller in src/lib.rs should NOT resolve to this one
    // (caller's file rule picks the local).
}

pub fn shared_name() {
    // intentionally empty — HOMONYM (3-way). See src/lib.rs.
}

pub fn two_way_ambig() {
    // intentionally empty — 2-way homonym. Caller in src/lib.rs
    // has no local `two_way_ambig`; resolver picks one of these
    // by "same crate root" rule.
}
