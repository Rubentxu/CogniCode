// ambig/mod.rs — third file with homonyms. Tests that the resolver
// doesn't pick THIS file's candidates when there's a local option.

pub fn compute(_x: i32) {
    // intentionally empty — single-candidate cross-file target from src/lib.rs.
}

pub fn shared_name() {
    // intentionally empty — HOMONYM (3-way). See src/lib.rs.
}

pub fn two_way_ambig() {
    // intentionally empty — 2-way homonym with nested/mod.rs.
}
