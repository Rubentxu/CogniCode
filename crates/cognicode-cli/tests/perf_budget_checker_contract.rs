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
//! the checker still reported success. Four of those nine became measurable in
//! this block; the declaration that tracks the remaining five is
//! `a_budgeted_operation_is_measured_or_declared_unmeasured`.
//!
//! That is a silent fallback. An operation with a budget and no measurement is
//! `Unknown`, and `Unknown` presented as clean is what `AGENTS.md` forbids
//! outright. It is also worse than having no budget: the file asserts a ceiling
//! that nobody is standing under.
//!
//! The second half of this file is about the opposite mistake, which cost a
//! full measurement run to find: a benchmark that **did** run, produced a
//! number, and was still reported as `UNMEASURED` because Criterion names a
//! benchmark inside a `benchmark_group` `group/function` and the parser only
//! ever compared `$2`. A gate that cannot see a measurement it paid for is
//! worse than one that sees nothing, because it names the wrong remedy.
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

use std::collections::BTreeSet;
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

// ---------------------------------------------------------------------------
// Reading the real repository
// ---------------------------------------------------------------------------
//
// Everything below reads the real files: the crate manifests, the checker's
// own `BENCH_SPECS`, and `perf-budget.toml`. The tests above this line supply
// synthetic inputs to exercise decision logic; these do the other job, which
// is to notice when the repository and its own declarations have drifted apart.
//
// The budget is 16 operations across three crates. It is entirely possible for
// the script's logic to be correct and its contents to be fiction, and no
// amount of running the script proves the second kind of wrong.

/// A `[[bench]]` target as the manifest declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct BenchTarget {
    krate: String,
    name: String,
    /// `harness = false`, read literally. Cargo's metadata does not report
    /// this field at all in this toolchain — not even for the known-good
    /// `graph_benchmarks` — so the manifest text is the only authority.
    harness_disabled: bool,
}

/// Every `[[bench]]` declared by a crate under `crates/`.
///
/// Parsed from the manifest text rather than via `cargo metadata`, so that a
/// contract test never has to take the build lock to describe a repository
/// whose whole point is that it is not currently building.
fn declared_bench_targets(root: &Path) -> Vec<BenchTarget> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join("crates"))
        .expect("read crates/")
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.is_dir() && p.join("Cargo.toml").is_file()).then_some(p)
        })
        .collect();
    dirs.sort();

    let mut out = Vec::new();
    for dir in dirs {
        let text = std::fs::read_to_string(dir.join("Cargo.toml")).expect("read crate manifest");
        let mut krate = String::new();
        let mut table = String::new();
        let mut current: Option<(String, bool)> = None;

        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') {
                // A header closes the table above it, so a bench is flushed
                // HERE and not at end-of-file. `cognicode-core` declares two
                // `[[bench]]` targets; a reader that only kept the last one
                // saw `fact_bridge_benchmarks` and reported `graph_benchmarks`
                // as undeclared — the reader, not the manifest, was wrong,
                // and the first run of this very test is what proved it.
                if let Some((name, harness_disabled)) = current.take() {
                    out.push(BenchTarget {
                        krate: krate.clone(),
                        name,
                        harness_disabled,
                    });
                }
                // `[[bench]]` and `[package]` both normalise to a bare name.
                table = line.trim_matches(|c| c == '[' || c == ']').to_string();
                current = (table == "bench").then(|| (String::new(), false));
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            if table == "package" && key == "name" {
                krate = value.trim_matches('"').to_string();
            } else if let Some(bench) = current.as_mut() {
                match key {
                    "name" => bench.0 = value.trim_matches('"').to_string(),
                    "harness" => bench.1 = value == "false",
                    _ => {}
                }
            }
        }

        if let Some((name, harness_disabled)) = current.take() {
            out.push(BenchTarget {
                krate,
                name,
                harness_disabled,
            });
        }
    }
    out
}

/// The `(crate, bench)` pairs `perf-budget-check.sh` actually runs.
///
/// Read from the script because the script is the thing that decides. The
/// entries are quoted one per line inside `BENCH_SPECS=( ... )`; matching the
/// opening line exactly keeps the prose in the comment block above it from
/// being mistaken for an entry.
fn registered_bench_specs(root: &Path) -> Vec<(String, String)> {
    let script = std::fs::read_to_string(root.join("scripts/perf-budget-check.sh"))
        .expect("perf-budget-check.sh is the checker these contracts are about");

    let mut out = Vec::new();
    let mut inside = false;
    for raw in script.lines() {
        let line = raw.trim();
        if !inside {
            inside = line == "BENCH_SPECS=(";
            continue;
        }
        if line == ")" {
            break;
        }
        let mut parts = line.trim_matches('"').split_whitespace();
        match (parts.next(), parts.next()) {
            (Some(krate), Some(bench)) => {
                out.push((krate.to_string(), bench.to_string()));
            }
            _ => continue,
        }
    }
    out
}

/// Every `bench_function("name")` in a benchmark source.
fn bench_function_names(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read benchmark source {}: {e}", path.display()));

    const MARKER: &str = "bench_function(\"";
    let mut names = Vec::new();
    let mut rest = text.as_str();
    while let Some(at) = rest.find(MARKER) {
        rest = &rest[at + MARKER.len()..];
        let Some(end) = rest.find('"') else { break };
        names.push(rest[..end].to_string());
        rest = &rest[end..];
    }
    names
}

/// Source path of a bench target, derived from the same manifest that
/// declared it.
fn bench_source(root: &Path, target: &BenchTarget) -> PathBuf {
    root.join("crates")
        .join(&target.krate)
        .join("benches")
        .join(format!("{}.rs", target.name))
}

/// The operation keys `perf-budget.toml` budgets, in file order.
fn budget_keys(root: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(root.join("perf-budget.toml"))
        .expect("perf-budget.toml is the budget these contracts are about");

    let mut section = String::new();
    let mut keys = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').to_string();
            continue;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !section.is_empty() && !key.trim().is_empty() && !value.trim().is_empty() {
            keys.push(key.trim().to_string());
        }
    }
    keys
}

/// The operations `perf-budget.toml` declares as having no benchmark.
///
/// A declaration, on its own comment line, in the same form the
/// `# ENFORCEMENT:` declaration uses: a fact somebody can read off the file
/// and a test can read back, rather than a sentence somebody has to interpret.
fn declared_unmeasured(root: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(root.join("perf-budget.toml"))
        .expect("perf-budget.toml is the budget these contracts are about");

    text.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("# UNMEASURED:"))
        .map(|rest| {
            rest.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_else(|| {
            panic!(
                "perf-budget.toml carries no `# UNMEASURED:` declaration. Without it there is \
                 no way to tell a budgeted operation nobody has written a benchmark for from a \
                 benchmark that was simply forgotten, and the header count is a number nobody \
                 can check."
            )
        })
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
             Note that running the checker is not free of consequence: while any \
             budgeted operation has no benchmark the run exits 3 UNMEASURED, so \
             making it blocking turns a lane permanently red. Today that set is \
             whatever `# UNMEASURED:` names — see \
             `a_budgeted_operation_is_measured_or_declared_unmeasured`."
        );
    }
}

// ---------------------------------------------------------------------------
// What the repository actually contains
// ---------------------------------------------------------------------------

#[test]
fn a_benchmark_inside_a_group_is_matched_by_its_function_name() {
    // The defect, reproduced. Criterion names a benchmark declared inside a
    // `benchmark_group` as `group/function`, and the parser used to compare
    // `$2` — the whole `group/function` string — against the budget key.
    //
    // Measured 2026-10-04 against a real run: the four `explorerql` benchmarks
    // executed, produced numbers in hundreds of nanoseconds, and the checker
    // printed all four as `UNMEASURED` and told the reader to "write the
    // benchmark" for a benchmark that had just run. The report was not
    // incomplete; it pointed at the wrong remedy.
    //
    // A test written only against bare names could not have caught this,
    // because the seven `graph.operations` benchmarks all use bare
    // `c.bench_function` and kept matching. It took writing the second bench
    // group in the workspace to expose it.
    let scratch = Scratch::new("grouped");
    let budget = scratch.write(
        "budget.toml",
        "[explorerql]\nparse_simple = 100\nexecute_find = 5000\n",
    );
    let bench = scratch.write(
        "bench.txt",
        &format!(
            "{}{}",
            criterion_line("explorerql_parse_simple/parse_simple", 495.0),
            criterion_line("explorerql_execute/execute_find", 82.0)
        ),
    );

    let out = run_checker(&repo_root(), &budget, &bench);
    let stdout = String::from_utf8_lossy(&out.stdout);

    for key in ["parse_simple", "execute_find"] {
        let row = stdout
            .lines()
            .find(|l| l.starts_with(key))
            .unwrap_or_else(|| {
                panic!(
                    "`{key}` is absent from the table; a grouped benchmark must be reported \
                     under its budget key, not under `group/{key}`:\n{stdout}"
                )
            });
        assert!(
            row.contains("PASS"),
            "a budgeted operation measured at {row:?} did not pass. Criterion's name is \
             `group/function`; the budget key is `function`, so the last path segment is \
             what has to be compared."
        );
    }

    assert!(
        !stdout.contains("UNMEASURED"),
        "a benchmark that ran was reported as never measured:\n{stdout}"
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "both budgeted operations were measured and within budget:\n{stdout}"
    );
}

#[test]
fn a_measured_benchmark_with_no_budget_key_is_named_not_dropped() {
    // The other half of the same defect. A measurement whose name matches no
    // budget entry used to vanish behind a bare `continue`, so a name that had
    // drifted off its key was indistinguishable from a benchmark that never
    // ran. That silence is what let the grouping bug read as "no benchmark
    // exists" instead of "your parser is wrong".
    let scratch = Scratch::new("unbudgeted");
    let budget = scratch.write("budget.toml", "[graph.operations]\nadd_node = 50\n");
    let bench = scratch.write(
        "bench.txt",
        &format!(
            "{}{}",
            criterion_line("add_node", 2_437.0),
            criterion_line("p99_call_graph/construction", 15_234.0)
        ),
    );

    let out = run_checker(&repo_root(), &budget, &bench);
    let stdout = String::from_utf8_lossy(&out.stdout);

    // The run is still a success: an exploratory benchmark outside the budget
    // is not a failure, and making it one would mean inventing a ceiling.
    assert_eq!(
        out.status.code(),
        Some(0),
        "an unbudgeted benchmark is not a failure:\n{stdout}"
    );

    assert!(
        stdout.contains("measured without a budget entry"),
        "a measurement that was taken and discarded was dropped silently, so a \
         benchmark name that drifted off its budget key looks exactly like one \
         that was never run:\n{stdout}"
    );
    assert!(
        stdout.contains("construction"),
        "the discarded measurement is counted but not named, which leaves the reader \
         to guess which benchmark drifted:\n{stdout}"
    );
}

#[test]
fn a_budgeted_operation_is_measured_or_declared_unmeasured() {
    // The header of `perf-budget.toml` used to assert "9 of the 16 entries have
    // no benchmark". That is a fact about a moving repository, written once, in
    // prose, with nothing checking it. It was already wrong the day the
    // ExplorerQL benchmarks landed.
    //
    // The declaration is now `# UNMEASURED: ...` — the same shape as the
    // `# ENFORCEMENT:` line — and this test recomputes the truth from the
    // benchmark sources rather than trusting the header. It fails in BOTH
    // directions, which is the part that makes it a gate rather than a note:
    //
    //   * a budget key with no benchmark and no declaration  -> RED
    //   * a benchmark written for a key, declaration not
    //     shrunk                                      -> RED
    //
    // The second direction is the one that matters after this block: when
    // somebody finally writes the `mcp.tools` benchmarks, forgetting to delete
    // them from the declaration is exactly the rot this pins.
    let root = repo_root();
    let registered = registered_bench_specs(&root);
    assert!(
        !registered.is_empty(),
        "no BENCH_SPECS entries were read out of scripts/perf-budget-check.sh. If the \
         variable was renamed, this test is asserting nothing — fix the reader and the \
         rename together."
    );

    let measured: BTreeSet<String> = registered
        .iter()
        .flat_map(|(krate, bench)| {
            let target = BenchTarget {
                krate: krate.clone(),
                name: bench.clone(),
                harness_disabled: true,
            };
            bench_function_names(&bench_source(&root, &target))
        })
        .collect();

    let computed: BTreeSet<String> = budget_keys(&root)
        .into_iter()
        .filter(|k| !measured.contains(k))
        .collect();

    let declared = declared_unmeasured(&root);
    let all: BTreeSet<String> = budget_keys(&root).into_iter().collect();

    // A declared operation that is not a budget key at all is a typo that would
    // otherwise read as a permanent exemption.
    let phantom: Vec<&String> = declared.difference(&all).collect();
    assert!(
        phantom.is_empty(),
        "`# UNMEASURED:` names {phantom:?}, which is not a key in perf-budget.toml. \
         A declaration about an operation that does not exist is an exemption nobody \
         can audit."
    );

    let undocumented: Vec<&String> = computed.difference(&declared).collect();
    let stale: Vec<&String> = declared.difference(&computed).collect();

    assert!(
        undocumented.is_empty() && stale.is_empty(),
        "perf-budget.toml's `# UNMEASURED:` declaration does not match the repository.\n\
         \n\
         declared but actually measured (delete from the declaration): {stale:?}\n\
         measured by nothing but not declared (add to the declaration): {undocumented:?}\n\
         \n\
         Measured across {} registered bench target(s): {registered:?}.\n\
         A declaration that can disagree with the code is a comment, not a fact.",
        registered.len()
    );
}

#[test]
fn every_registered_bench_target_exists_and_really_measures() {
    // `BENCH_SPECS` is the list of benchmarks the checker runs, and the loop
    // around it deliberately tolerates a failure so one broken target cannot
    // cost the others their measurements. That tolerance has a cost, and this
    // test is what pays it: with it, a renamed or deleted target is a red test
    // here instead of a permanent, silent `UNMEASURED` in a 457-second run
    // that nobody is going to run again.
    //
    // It also pins `harness = false`, which is not a style preference. Without
    // it Cargo builds the target with `libtest`, `criterion_main!` never runs,
    // the binary prints "0 tests, 0 benchmarks" and exits **0** — a benchmark
    // that appears to have run and measured nothing at all.
    let root = repo_root();
    let targets = declared_bench_targets(&root);
    let registered = registered_bench_specs(&root);
    assert!(
        !targets.is_empty(),
        "no [[bench]] targets were parsed out of crates/*/Cargo.toml. If the manifests \
         were restructured, this test is asserting nothing."
    );

    for (krate, bench) in &registered {
        let found = targets
            .iter()
            .find(|t| &t.krate == krate && &t.name == bench)
            .unwrap_or_else(|| {
                panic!(
                    "BENCH_SPECS runs `{krate}/{bench}`, which no manifest declares. \
                     `cargo bench --bench {bench}` will fail, the loop will log it and \
                     carry on, and every budgeted operation it was meant to measure will \
                     report UNMEASURED from now on with nothing to trace it to. \
                     Declared targets are: {targets:?}"
                )
            });

        assert!(
            found.harness_disabled,
            "BENCH_SPECS runs `{krate}/{bench}`, whose [[bench]] does not set \
             `harness = false`. Cargo builds it with libtest, `criterion_main!` never \
             executes, the binary prints \"0 tests, 0 benchmarks\" and exits 0, and the \
             checker reads that as a clean run with nothing measured."
        );

        let source = bench_source(&root, found);
        assert!(
            source.is_file(),
            "`{krate}/{bench}` is declared but `{}` does not exist",
            source.display()
        );
    }
}

#[test]
fn a_benchmark_written_for_a_budgeted_operation_is_registered() {
    // The mirror image of the previous test. Writing the benchmark is only
    // half of it: `perf-budget-check.sh` runs the targets in `BENCH_SPECS`, so
    // a benchmark living in a file that nobody registers measures nothing and
    // still leaves its budgeted operation reporting `UNMEASURED` forever.
    //
    // This is a real shape rather than a hypothetical one: the workspace has
    // three bench targets and the checker ran one of them for most of its
    // life.
    let root = repo_root();
    let registered = registered_bench_specs(&root);
    let budgeted: BTreeSet<String> = budget_keys(&root).into_iter().collect();

    for target in declared_bench_targets(&root) {
        let is_registered = registered
            .iter()
            .any(|(k, b)| k == &target.krate && b == &target.name);
        if is_registered {
            continue;
        }

        let source = bench_source(&root, &target);
        if !source.is_file() {
            continue;
        }

        let orphan: Vec<String> = bench_function_names(&source)
            .into_iter()
            .filter(|n| budgeted.contains(n))
            .collect();
        if orphan.is_empty() {
            continue;
        }

        panic!(
            "{}/{} measures the budgeted operation(s) {orphan:?}, but the checker does not \
             run this target: it is absent from BENCH_SPECS in scripts/perf-budget-check.sh. \
             Writing the benchmark is only half the job — without the registration these \
             operations keep reporting UNMEASURED and every measurement is discarded.\n\
             \n\
             If this target is deliberately not run by the checker, its file has to say so \
             and say why, and the declarations above have to agree.",
            target.krate, target.name
        );
    }
}
