#!/usr/bin/env python3
"""Generate the PRF-ANA-07 massive-collision corpus.

Layout (flat sibling files at src/ alongside lib.rs):
  src/lib.rs              caller_in_lib + local `init`
  src/d{1..50}.rs         each declares `pub fn init()` (50 decoys)
  src/sibling_unique_compute.rs pub fn compute()  (single cross-file)

The "all functions are top-level at the crate root" structure means
each `init()` lives at the FQN `crate::init`, and the resolver's
`by_name["init"]` map has 51 entries all pointing to distinct
`SymbolId`s (one per file). That is exactly the case PRF-ANA-07
asks about: massively-distributed homonym collisions.

Two test cases (this corpus intentionally does NOT cover the
no-local-anchor ambiguous-drop case — that case is already
exercised by `cross_file_scope_aware/` corpus from F2.W7
via `w7_two_way_homonym_no_caller_file_honest_drop`. Adding
it here would require Rust's parser to accept an unqualified
`dispatch()` call when multiple modules export it, which it
won't — and putting a local `dispatch()` dummy would hide the
ambiguity we want to test):

  (1) `init()` called from caller_in_lib — visibility rule MUST
      pick the local despite 50 sibling homonyms.

  (2) `sibling_unique_compute()` cross-file, single candidate.

Critical detail: lib.rs has `mod d1; mod d2; ... mod d50; mod
sibling_unique_compute;` so each sibling file becomes a top-level
module. The function inside each is `pub fn init()` (or
`compute()`) at the module path `<crate>::<mod_name>::<fn_name>`.
"""
from pathlib import Path

CORPUS_ROOT = Path("docs/prf/fixtures/massive_collision_corpus")
SRC_DIR = CORPUS_ROOT / "src"


def main() -> None:
    SRC_DIR.mkdir(parents=True, exist_ok=True)

    modules = []
    for i in range(1, 51):
        mod_name = f"d{i}"
        modules.append(mod_name)
        (SRC_DIR / f"{mod_name}.rs").write_text(
            f"pub fn init() {{ /* homonym {i} */ }}\n"
        )
    modules.append("sibling_unique_compute")
    (SRC_DIR / "sibling_unique_compute.rs").write_text(
        "pub fn compute() { /* single candidate cross-file */ }\n"
    )

    mod_decls = "\n".join(f"mod {m};" for m in modules)

    (SRC_DIR / "lib.rs").write_text(
        f"""{mod_decls}

pub fn caller_in_lib() {{
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
}}

fn init() {{
    // LOCAL version of init. The visibility rule picks this over the
    // 50 sibling `d*::init()` homonyms, regardless of how many
    // candidates the resolver sees globally. The empty body keeps the
    // corpus cheap to parse.
}}
"""
    )

    file_count = sum(1 for _ in SRC_DIR.iterdir())
    print(
        f"Generated corpus at {CORPUS_ROOT}/. Files: {file_count} "
        f"(lib.rs + {len(modules)} modules)."
    )


if __name__ == "__main__":
    main()
