//! PR-SEC / CP2.8 — the performance-budget checker must not report success
//! for operations it never measured.
//!
//! ## The defect this pins
//!
//! `scripts/perf-budget-check.sh` printed, for every budget entry that no
//! benchmark produced:
//!
//! ```text
//! graph_search    10000    (missing)   SKIP
//! ```
//!
//! and then, after counting only the benchmarks it *did* run, printed
//!
//! ```text
//! === All benchmarks within budget ===
//! ```
//!
//! and exited 0. Measured on 2026-10-02 against the real budget: **9 of the 16
//! budgeted operations have no benchmark anywhere in `crates/*/benches/`**, and
//! the checker still reported success.
//!
//! That is a silent fallback. An operation with a budget and no measurement is
//! `Unknown`, and `Unknown` presented as clean is what `AGENTS.md` forbids
//! outright. It is also worse than having no budget: the file asserts a ceiling
//! that nobody is standing under.
//!
//! ## Why these tests do not run the benchmarks
//!
//! `cargo bench` for this suite takes **457 s**. Paying that per assertion is
//! not a test, it is a tax, and the reasons are already enumerated in N+56:
//! a check that is too expensive to run does not get run. So the script takes
//! its raw benchmark output from `COGNICODE_PERF_BENCH_OUTPUT` when set, and
//! the budget from `COGNICODE_PERF_BUDGET_FILE`. The decision logic under test
//! is the real one, unchanged; only its two inputs are supplied.
//!
//! The exit codes are the contract:
//!
//! | code | meaning                                        |
//! |------|------------------------------------------------|
//! | 0    | every budgeted operation was measured and within |
//! | 1    | a measured operation exceeded its budget          |
//! | 3    | a budgeted operation was never measured           |

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

/// A scratch directory that cleans itself up, so a failing assertion does not
/// leave benchmark fixtures behind in `/tmp`.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("perf-budget-contract-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        Scratch(dir)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, contents).expect("write fixture");
        p
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Criterion's `bencher` output, which is what the script's parser consumes.
/// Values are nanoseconds; the script normalises them to microseconds.
fn criterion_line(name: &str, ns: f64) -> String {
    format!("test {name} ... bench: {ns:>18} ns/iter (+/- 5)\n")
}

/// Run the real script against a synthetic budget and benchmark output.
fn run_checker(root: &Path, budget: &Path, bench: &Path) -> Output {
    Command::new("bash")
        .arg(root.join("scripts/perf-budget-check.sh"))
        .current_dir(root)
        .env("COGNICODE_PERF_BUDGET_FILE", budget)
        .env("COGNICODE_PERF_BENCH_OUTPUT", bench)
        .output()
        .expect("cannot run bash")
}

#[test]
fn a_budgeted_operation_with_no_benchmark_is_not_success() {
    // The defect, reproduced. `graph_search` has a budget and no measurement.
    let scratch = Scratch::new("unmeasured");
    let budget = scratch.write(
        "budget.toml",
        "[graph.operations]\nadd_node = 50\n\n[mcp.tools]\ngraph_search = 10000\n",
    );
    let bench = scratch.write(
        "bench.txt",
        &format!(
            "{}{}",
            criterion_line("add_node", 2_437.0),
            criterion_line("bfs", 53_092.0)
        ),
    );

    let out = run_checker(&repo_root(), &budget, &bench);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert_ne!(
        out.status.code(),
        Some(0),
        "the checker reported success with a budgeted operation it never measured\n\
         --- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );

    // The verdict must name what was not measured. A non-zero exit that says
    // nothing is a gate nobody can act on.
    let verdict = format!("{stdout}{stderr}");
    assert!(
        verdict.contains("graph_search"),
        "the checker failed without naming the unmeasured operation, so there is nothing \
         to fix. Output was:\n{verdict}"
    );
    assert!(
        verdict.to_lowercase().contains("unmeasured")
            || verdict.to_lowercase().contains("no benchmark")
            || verdict.contains("not measured"),
        "the checker did not distinguish 'not measured' from 'within budget'. \
         An unmeasured operation is Unknown and must not read as clean. Output was:\n{verdict}"
    );
    assert!(
        !verdict.contains("All benchmarks within budget"),
        "the checker still printed its success banner while 1 operation was unmeasured:\n{verdict}"
    );
}

#[test]
fn an_measured_operation_over_budget_is_a_different_failure() {
    // Over-budget and unmeasured are different diagnoses and must not collapse
    // into one undifferentiated red.
    let scratch = Scratch::new("over");
    let budget = scratch.write(
        "budget.toml",
        "[graph.operations]\nadd_node = 50\nbfs_traversal_100_nodes = 500\n",
    );
    let bench = scratch.write(
        "bench.txt",
        &format!(
            "{}{}",
            criterion_line("add_node", 2_437.0),
            criterion_line("bfs_traversal_100_nodes", 9_000_000.0)
        ),
    );

    let out = run_checker(&repo_root(), &budget, &bench);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_eq!(
        out.status.code(),
        Some(1),
        "an operation measured over budget must exit 1, not the unmeasured code\n{stdout}"
    );
    assert!(
        stdout.contains("over budget"),
        "the over-budget verdict is not reported as such:\n{stdout}"
    );
}

#[test]
fn a_fully_measured_budget_within_limits_is_the_only_success() {
    let scratch = Scratch::new("clean");
    let budget = scratch.write(
        "budget.toml",
        "[graph.operations]\nadd_node = 50\nshortest_path = 1000\n",
    );
    let bench = scratch.write(
        "bench.txt",
        &format!(
            "{}{}",
            criterion_line("add_node", 2_437.0),
            criterion_line("shortest_path", 304_464.0)
        ),
    );

    let out = run_checker(&repo_root(), &budget, &bench);
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert_eq!(
        out.status.code(),
        Some(0),
        "every budgeted operation was measured and within budget, so this is a success:\n{stdout}"
    );
    assert!(
        stdout.contains("All benchmarks within budget"),
        "a clean run must say so:\n{stdout}"
    );
}

#[test]
fn the_real_budget_declares_enforcement_it_does_not_have() {
    // `perf-budget.toml` opened with "CI fails if any operation exceeds its
    // budget" while no workflow referenced `perf` and nothing invoked the
    // script. A file that promises a ceiling nobody stands under is worse than
    // no budget at all.
    //
    // The first version of this test grepped for the phrase. It was wrong: once
    // the false claim was replaced by an explanation *quoting* it, the grep
    // matched the denial and failed. A substring cannot tell an assertion from
    // a refutation of an assertion, so the status is now declared on its own
    // line and parsed, which is the same discipline the rest of this file uses.
    //
    // Requiring the declaration also means deleting it fails loudly, instead of
    // the check quietly passing over an undeclared file.
    let root = repo_root();
    let budget = std::fs::read_to_string(root.join("perf-budget.toml"))
        .expect("perf-budget.toml is the budget this contract is about");

    let declared = budget
        .lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("# ENFORCEMENT:"))
        .map(|v| v.trim().to_ascii_lowercase())
        .expect(
            "perf-budget.toml carries no `# ENFORCEMENT:` declaration. Add one so the \
             enforcement status is a fact rather than a sentence somebody has to interpret.",
        );

    assert!(
        declared == "ci" || declared == "none",
        "unrecognised enforcement status `{declared}`; expected `ci` or `none`"
    );

    // Which lane runs the checker, and whether it can stop that lane. The
    // orchestrator that gates a merge is `merge-gate.pipeline.kts`; it used to
    // be `.github/workflows/pr-ci.yml`, and the declaration is about a lane
    // rather than about a file, so it is asked of every pipeline instead of one
    // named path.
    //
    // `ENFORCEMENT` is about enforcement, not about invocation, and the two came
    // apart here. `certification.pipeline.kts` runs the checker, prints the
    // exit code, says in so many words that the verdict is not a PASS, and
    // continues — so the budget is *observed* and not *enforced*, and `none`
    // is still the truthful word. The previous version of this test asked only
    // "is the checker invoked?", which cannot tell those apart, so it read a
    // faithful lane as a lie. It stayed green anyway, because the reader it used
    // matched `sh(` and the command on ONE line and could not see inside the
    // multi-line `sh("""…""")` body the stage actually uses. Two defects, one of
    // them in the instrument.
    let invocations = common::invocations("perf-budget-check");
    let observed: Vec<&str> = invocations.iter().map(|i| i.at.as_str()).collect();
    let enforced: Vec<&str> = invocations
        .iter()
        .filter(|i| i.blocking)
        .map(|i| i.at.as_str())
        .collect();

    // Both directions, because the old one-way form could not fail in the
    // state the repository is in: `perf-budget.toml` declares `none`, and
    // `declared != "ci" || wired` is then true whatever `wired` says. An
    // assertion that cannot go red in the present is not a gate.
    if declared == "ci" {
        assert!(
            !enforced.is_empty(),
            "perf-budget.toml declares `ENFORCEMENT: ci`, but no pipeline runs \
             the checker in a position where its verdict can fail the lane \
             (observed in {observed:?}, none of them blocking). Either wire it \
             where a non-zero exit stops the lane or change the declaration in \
             the same commit."
        );
    } else {
        assert!(
            enforced.is_empty(),
            "perf-budget.toml declares `ENFORCEMENT: none`, yet the checker can \
             fail the lane from {enforced:?}. Either the declaration is stale or \
             the lane is enforcing something the budget says is not enforced. \
             Note that running the checker is not free of consequence: with 9 of \
             the 16 budgeted operations having no benchmark it exits 3 \
             UNMEASURED, so making it blocking turns a lane permanently red."
        );
    }
}
