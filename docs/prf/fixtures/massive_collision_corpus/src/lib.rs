mod d1;
mod d2;
mod d3;
mod d4;
mod d5;
mod d6;
mod d7;
mod d8;
mod d9;
mod d10;
mod d11;
mod d12;
mod d13;
mod d14;
mod d15;
mod d16;
mod d17;
mod d18;
mod d19;
mod d20;
mod d21;
mod d22;
mod d23;
mod d24;
mod d25;
mod d26;
mod d27;
mod d28;
mod d29;
mod d30;
mod d31;
mod d32;
mod d33;
mod d34;
mod d35;
mod d36;
mod d37;
mod d38;
mod d39;
mod d40;
mod d41;
mod d42;
mod d43;
mod d44;
mod d45;
mod d46;
mod d47;
mod d48;
mod d49;
mod d50;
mod sibling_unique_compute;

pub fn caller_in_lib() {
    // 1. Mass-collision same-name: 50 sibling modules (d1..d50) each
    //    declare a top-level `init()`. Visibility rule MUST pick the
    //    local `init` declared in this file over all 50 sibling
    //    homonyms. The resolver sees a `by_name["init"]` map with 51
    //    entries; visibility filters down to 1 (the local). If the
    //    resolver regressed to "first inserted" or "lex FQN", the
    //    target would be one of the sibling modules instead.
    init();

    // 2. Single-candidate cross-file: only `sibling_unique_compute`
    //    declares `compute`. Single-candidate rule picks it (no
    //    homonyms). This is the same coverage as F2.W7's
    //    `single_candidate_cross_file_resolves_to_ambig_compute` but
    //    on this corpus, which exercises a much larger fan-out so a
    //    local-regression in index construction would surface here.
    sibling_unique_compute::compute();
}

fn init() {
    // LOCAL version of init. The visibility rule picks this over the
    // 50 sibling `d*::init()` homonyms, regardless of how many
    // candidates the resolver sees globally. The empty body keeps the
    // corpus cheap to parse.
}
