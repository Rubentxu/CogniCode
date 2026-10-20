// src/lib.rs — caller in lib. Has one local helper for homonym tests.

pub mod nested;
pub mod ambig;

pub fn caller_in_lib() {
    // 1. Cross-file call, 1 candidate: caller in lib, callee in nested.
    //    Parser strips `nested::` → leaf = `callee_in_nested`.
    //    Resolver: 1 candidate globally → picks nested::callee_in_nested.
    nested::callee_in_nested();

    // 2. Homonym, caller's file wins:
    //    `local_helper` lives in BOTH src/lib.rs (local) and nested/mod.rs.
    //    Caller is in src/lib.rs → "multiple in caller's file" rule
    //    picks src/lib.rs::local_helper.
    local_helper();

    // 3. Single-candidate cross-file: only ambig/ declares `compute`.
    //    Parser strips `ambig::` → leaf = `compute`.
    //    Resolver: 1 candidate globally → picks ambig::compute.
    ambig::compute(42);

    // 4. Genuine cross-file ambiguity (3-way):
    //    `shared_name` lives in THREE files: src/lib.rs, nested/mod.rs, ambig/mod.rs.
    //    Caller is in src/lib.rs → "multiple in caller's file" rule
    //    picks src/lib.rs::shared_name (LOCAL).
    //    This validates that the resolver doesn't accidentally pick
    //    a cross-file homonym when a local one exists.
    shared_name();

    // 5. Genuine cross-file ambiguity (2-way, NO local):
    //    `two_way_ambig` lives ONLY in nested/mod.rs and ambig/mod.rs.
    //    Caller is in src/lib.rs. Neither file is the caller's file.
    //    Both are in the same crate root → "same crate root" rule
    //    picks one (deterministic but content-based).
    nested::two_way_ambig();
}

fn local_helper() {
    // intentionally empty — local target for caller.
}

pub fn shared_name() {
    // intentionally empty — local target for caller.
}
