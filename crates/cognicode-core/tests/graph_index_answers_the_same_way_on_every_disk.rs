//! The index must not answer differently because of the order it was fed.
//!
//! The release candidate gate found this. Its `clean-clone` stage failed
//! with one test red out of 3159: the on-demand graph was asked for the
//! symbol `new` and answered with no callees and no callers. The same
//! tree, byte for byte, answered with five of each on the machine that
//! ran the battery. The two trees held the same 690 `.rs` files with
//! the same bytes in each of them.
//!
//! The cause is the order. The index is filled in whatever order the
//! sources arrive — the walk on a directory, the caller's list on the
//! in-memory path — and an ambiguous name resolves to its FIRST
//! definition. `new` is defined 299 times in this crate, so which one
//! comes first decides the answer. On the machine that ran the battery
//! it was `src/findings/mod.rs:123`, which has edges; on the clean
//! clone, which lives on a different filesystem, it was
//! `testdata/u10_compat_corpus/src/lib.rs:4`, a corpus fixture whose
//! `new` calls nothing.
//!
//! That filesystem dependence is the part that matters, and it is not a
//! detail of one filesystem. Measured on this machine, the same
//! directory of 64 files comes back in creation order from the ext4
//! volume and in a different order from tmpfs. A query whose answer
//! depends on the disk is not an answer.
//!
//! What this test pins is the property the index owes its callers: the
//! same set of sources produces the same index, whatever order they
//! arrive in.

use cognicode_core::infrastructure::graph::{
    LightweightIndex, OnDemandGraphBuilder, TraversalDirection,
};

/// Big enough that a filesystem's own order cannot coincide with the
/// lexicographic one by luck.
const FILES: usize = 64;

const SYMBOL: &str = "a_symbol_defined_in_many_files";

fn sources() -> Vec<(String, String)> {
    (0..FILES)
        .map(|i| (format!("f{i:02}.rs"), format!("pub fn {SYMBOL}() {{}}\n")))
        .collect()
}

#[test]
fn the_index_does_not_depend_on_the_order_the_sources_arrive_in() {
    let all = sources();

    let mut forward = LightweightIndex::new();
    forward.build_from_sources(all.iter().map(|(f, s)| (f.as_str(), s.as_str())));

    let mut backward = LightweightIndex::new();
    backward.build_from_sources(all.iter().rev().map(|(f, s)| (f.as_str(), s.as_str())));

    assert_eq!(
        forward.find_symbol(SYMBOL),
        backward.find_symbol(SYMBOL),
        "the index kept the order the sources arrived in, so the same \
         sources indexed differently answer differently"
    );
}

#[test]
fn the_index_lists_the_locations_of_a_symbol_in_path_order() {
    let mut index = LightweightIndex::new();
    index.build_from_sources(sources().iter().map(|(f, s)| (f.as_str(), s.as_str())));

    let locations = index.find_symbol(SYMBOL);
    assert_eq!(locations.len(), FILES, "every file defines the symbol");

    let mut sorted = locations.clone();
    sorted.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    assert_eq!(
        locations, sorted,
        "the locations of a symbol are not in path order, so which one a \
         query picks depends on the order the sources arrived in"
    );
}

#[test]
fn an_ambiguous_symbol_resolves_to_the_same_definition_on_every_filesystem() {
    let tmp = tempfile::tempdir().unwrap();
    // Created in DESCENDING name order, so the creation order is the
    // reverse of the lexicographic one.
    for i in (0..FILES).rev() {
        std::fs::write(
            tmp.path().join(format!("f{i:02}.rs")),
            format!("pub fn {SYMBOL}() {{}}\n"),
        )
        .unwrap();
    }

    let mut builder = OnDemandGraphBuilder::new();
    builder.set_index(tmp.path()).unwrap();

    let result = builder.build_for_symbol(SYMBOL, 1, TraversalDirection::Callees);

    let expected = tmp.path().join("f00.rs").to_string_lossy().to_string();
    assert_eq!(
        result.root_symbol.location().file(),
        expected,
        "the index resolved an ambiguous symbol to whichever definition the \
         filesystem happened to return first, so the same query answers \
         differently on different disks"
    );
}
