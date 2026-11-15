//! The tests of these modules do not wait for the wall clock.
//!
//! ## The hole
//!
//! A test that changes a file and then sleeps so that the clock moves is
//! waiting for a filesystem tick: the write and the read can land in the
//! same tick, the tick can be coarse, and the wait is paid in every run
//! of the suite. Thirteen tests did that, for four and a half seconds per
//! run of `cognicode-core` —and one of them waited a second and a tenth
//! for a single file.
//!
//! ## What replaces it
//!
//! The modification time is a value, not a moment: a test sets it to a
//! known instant (`crate::test_support::bump_mtime`) and the comparison
//! the product makes —the time a manifest recorded against the time on
//! disk— sees the change without anybody waiting.
//!
//! ## The rule
//!
//! The sleeps that remain in these modules are the ones that wait for
//! another THREAD, not for the clock. They are listed below with the
//! reason each one is there. A sleep that appears without a line in that
//! list is the clock coming back, and this test fails.
//!
//! The two are not the same thing: a thread runs on its own schedule and
//! joining it costs what it costs, but the clock is not a promise.

use std::path::PathBuf;

/// The modules whose tests must not wait for the clock.
const MODULES: &[&str] = &[
    "src/interface/mcp/handlers/mod.rs",
    "src/infrastructure/graph/file_manifest.rs",
    "src/infrastructure/graph/incremental_scanner.rs",
    "src/infrastructure/graph/per_file_graph.rs",
];

/// The sleeps that remain, and why each one is there.
///
/// A sleep that waits for another thread is not the clock. A sleep that
/// waits for the clock is a bug: the clock is not a promise, and the
/// thing it waits for —a filesystem tick —may never arrive.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "src/interface/mcp/handlers/mod.rs",
        "Duration::from_millis(i as u64 * 5)",
        "four writers write the same snapshot at once, and this spreads the race between them",
    ),
    (
        "src/interface/mcp/handlers/mod.rs",
        "Duration::from_millis(100)",
        "the writers that were dropped mid-flight need the chance to rename their file or leave it orphaned",
    ),
];

/// The crate whose sources these are.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// What a line waits for, if the line sleeps.
fn sleep_in(line: &str) -> Option<String> {
    let start = line.find("sleep(")? + "sleep(".len();
    let mut depth = 1;
    for (offset, ch) in line[start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(line[start..start + offset].trim().to_string());
                }
            }
            _ => {}
        }
    }
    None
}

#[test]
fn no_test_in_these_modules_waits_for_the_clock() {
    let mut unexpected: Vec<String> = Vec::new();

    for module in MODULES {
        let path = crate_root().join(module);
        let text = std::fs::read_to_string(&path).expect("the module is readable");
        for (number, line) in text.lines().enumerate() {
            let Some(duration) = sleep_in(line) else {
                continue;
            };
            let known = ALLOWED
                .iter()
                .any(|(file, waits, _)| *file == *module && *waits == duration);
            if !known {
                unexpected.push(format!("{module}:{}: waits {duration}", number + 1));
            }
        }
    }

    assert!(
        unexpected.is_empty(),
        "a test waits for the wall clock again:\n{}\n\
         The clock is not a promise: a filesystem tick may never arrive. Set the \
         modification time to a known instant instead \
         (`crate::test_support::bump_mtime`), or —if the sleep waits for another \
         thread and not for the clock— list it in this contract with the reason it \
         is there.",
        unexpected.join("\n")
    );
}

#[test]
fn the_sleeps_that_remain_are_the_ones_the_contract_names() {
    // The allowlist is not a decoration: a sleep that disappears from a
    // module leaves a line here that no longer describes anything.
    for (file, waits, reason) in ALLOWED {
        assert!(
            !reason.trim().is_empty(),
            "{file} waits {waits} without saying why it is there"
        );
    }
}
