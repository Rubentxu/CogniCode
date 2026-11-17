//! Native metrics for Rust, Java, Kotlin and Go, pinned by fixtures.
//!
//! Every expected number below was derived by hand from the metric
//! definitions in `domain::metrics`, not copied from the output. Each fixture
//! exists to pin one semantic exception: what a `match`/`when`/`switch` arm
//! costs, what a closure belongs to, what an `else if` weighs.

use cognicode_core::domain::metrics::{
    FileMetrics, FunctionMetrics, MetricKind, MetricSupport, MetricsCoverage, MetricsOutcome,
};
use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::native_metrics::{measure_by_extension, measure_source};

fn measured(language: Language, path: &str, source: &str) -> FileMetrics {
    match measure_source(language, path, source) {
        MetricsOutcome::Measured { metrics } => *metrics,
        other => panic!("expected a measurement, got {other:?}"),
    }
}

fn function<'m>(report: &'m FileMetrics, qualified: &str) -> &'m FunctionMetrics {
    report
        .functions
        .iter()
        .find(|f| match &f.container {
            Some(c) => format!("{c}.{}", f.name) == qualified,
            None => f.name == qualified,
        })
        .unwrap_or_else(|| {
            let known: Vec<_> = report.functions.iter().map(|f| f.id.clone()).collect();
            panic!("no function {qualified}; measured {known:?}")
        })
}

/// `(cyclomatic, cognitive, nesting)` of a function.
fn shape(report: &FileMetrics, qualified: &str) -> (u32, u32, u32) {
    let f = function(report, qualified);
    (f.cyclomatic, f.cognitive, f.nesting_depth)
}

const RUST: &str = r#"
fn plain(a: i32) -> i32 {
    a + 1
}

fn classify(n: i32) -> &'static str {
    match n {
        0 => "zero",
        1 | 2 => "small",
        x if x > 100 => "huge",
        _ => "other",
    }
}

fn guarded_wildcard(n: i32) -> i32 {
    match n {
        _ if n > 0 => 1,
        _ => 0,
    }
}

fn parse(s: &str) -> Option<i32> {
    let n = s.parse::<i32>().ok()?;
    let Some(m) = n.checked_add(1) else {
        return None;
    };
    Some(m)
}

fn outer(v: Vec<i32>) -> i32 {
    fn helper(x: i32) -> i32 {
        if x > 0 { 1 } else { 0 }
    }
    v.iter().map(|x| if *x > 0 { 1 } else { 0 }).sum()
}

fn chain(a: i32) -> i32 {
    if a == 1 {
        1
    } else if a == 2 {
        2
    } else {
        3
    }
}

fn nested(a: bool, b: bool) -> i32 {
    if a {
        if b { 1 } else { 2 }
    } else {
        3
    }
}

fn flags(a: bool, b: bool, c: bool, d: bool) -> bool {
    a && b && c || d
}

fn loops(n: u32) {
    while n > 0 {
        for _ in 0..n {}
    }
}

fn doc() {
    // a comment

    let a = 1;
    /* block
       comment */
    let _b = a;
}

struct Shape;
impl Shape {
    fn area(&self, w: u32, h: u32) -> u32 {
        w * h
    }
}
"#;

#[test]
fn rust_a_straight_line_function_is_one_path_and_costs_nothing_cognitively() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    assert_eq!(shape(&report, "plain"), (1, 0, 0));
    assert_eq!(function(&report, "plain").size.parameters, 1);
}

#[test]
fn rust_match_counts_each_arm_but_not_the_wildcard_and_is_one_cognitive_step() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    // 1 + `0`, `1 | 2` (one or-pattern), guarded `x` = 4; `_` adds nothing.
    assert_eq!(shape(&report, "classify"), (4, 1, 1));
}

#[test]
fn rust_a_guarded_wildcard_is_a_real_branch() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    assert_eq!(shape(&report, "guarded_wildcard"), (2, 1, 1));
}

#[test]
fn rust_question_mark_and_let_else_are_decisions() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    // 1 + `?` + `let ... else`; the let-else costs 1 cognitively, `?` nothing.
    assert_eq!(shape(&report, "parse"), (3, 1, 0));
}

#[test]
fn rust_a_nested_fn_is_its_own_unit_and_a_closure_belongs_to_the_function() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    // The closure's `if` nests one level (+2) and its `else` is +1.
    assert_eq!(shape(&report, "outer"), (2, 3, 1));
    // The helper is measured alone, and its decision is not outer's.
    assert_eq!(shape(&report, "outer.helper"), (2, 2, 1));
}

#[test]
fn rust_fan_out_counts_distinct_callees_of_the_function_only() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    assert_eq!(
        function(&report, "outer").callees,
        [".iter", ".map", ".sum"]
    );
    assert_eq!(function(&report, "parse").fan_out, 4);
    assert_eq!(function(&report, "plain").fan_out, 0);
}

#[test]
fn rust_else_if_and_else_are_flat_increments() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    assert_eq!(shape(&report, "chain"), (3, 3, 1));
}

#[test]
fn rust_nesting_weighs_the_inner_decisions() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    // if 1 + inner if (1 + 1 nesting) + inner else 1 + outer else 1.
    assert_eq!(shape(&report, "nested"), (3, 5, 2));
    // while 1 + for (1 + 1).
    assert_eq!(shape(&report, "loops"), (3, 3, 2));
}

#[test]
fn rust_a_run_of_one_boolean_operator_counts_once() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    // `(a && b && c) || d`: three operators, two runs.
    assert_eq!(shape(&report, "flags"), (4, 2, 0));
}

#[test]
fn rust_code_lines_exclude_blank_and_comment_only_lines() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    let doc = function(&report, "doc");
    assert_eq!(doc.size.lines, 8);
    assert_eq!(doc.size.code_lines, 4);
}

#[test]
fn rust_a_method_is_qualified_by_its_impl_type() {
    let report = measured(Language::Rust, "src/lib.rs", RUST);
    let area = function(&report, "Shape.area");
    assert_eq!(area.container.as_deref(), Some("Shape"));
    assert!(area.id.starts_with("src/lib.rs:Shape.area:"), "{}", area.id);
    // `&self` is a parameter, as the legacy count has always taken it.
    assert_eq!(area.size.parameters, 3);
}

const JAVA: &str = r#"
class Shop {
    int legacy(int k) {
        switch (k) {
            case 1:
            case 2:
                return 1;
            case 3:
                return 3;
            default:
                return 0;
        }
    }

    int arrow(int k) {
        return switch (k) {
            case 1, 2 -> 1;
            case 3 -> 3;
            default -> 0;
        };
    }

    int tern(boolean a) {
        return a ? 1 : 2;
    }

    void guarded() {
        try {
            work();
        } catch (Exception e) {
            log(e);
        }
    }

    void labelled(int n) {
        outer:
        for (int i = 0; i < n; i++) {
            for (int j = 0; j < n; j++) {
                if (j == 1) continue outer;
            }
        }
    }
}

interface Port {
    int get();
}
"#;

#[test]
fn java_legacy_stacked_case_labels_count_one_branch_each() {
    let report = measured(Language::Java, "Shop.java", JAVA);
    // Labels `1`, `2`, `3` are three ways in; `default` adds nothing.
    assert_eq!(shape(&report, "Shop.legacy"), (4, 1, 1));
}

#[test]
fn java_an_arrow_case_with_several_constants_is_one_branch() {
    let report = measured(Language::Java, "Shop.java", JAVA);
    assert_eq!(shape(&report, "Shop.arrow"), (3, 1, 1));
}

#[test]
fn java_a_ternary_is_a_decision() {
    let report = measured(Language::Java, "Shop.java", JAVA);
    assert_eq!(shape(&report, "Shop.tern"), (2, 1, 0));
}

#[test]
fn java_catch_is_a_decision_and_try_is_a_nesting_level() {
    let report = measured(Language::Java, "Shop.java", JAVA);
    assert_eq!(shape(&report, "Shop.guarded"), (2, 1, 1));
    assert_eq!(function(&report, "Shop.guarded").callees, ["log", "work"]);
}

#[test]
fn java_a_labelled_jump_costs_one_more() {
    let report = measured(Language::Java, "Shop.java", JAVA);
    // for 1 + for 2 + if 3 + `continue outer` 1; for, for, if are decisions.
    assert_eq!(shape(&report, "Shop.labelled"), (4, 7, 3));
}

#[test]
fn java_a_bodiless_declaration_has_nothing_to_measure() {
    let report = measured(Language::Java, "Shop.java", JAVA);
    assert!(report.functions.iter().all(|f| f.name != "get"));
}

const KOTLIN: &str = r#"
class Box(val n: Int) {
    constructor() : this(0) {
        println("zero")
    }

    fun kind(x: Int): String = when (x) {
        1, 2 -> "a"
        3 -> "b"
        else -> "c"
    }

    fun pick(a: String?): String {
        return a ?: "none"
    }

    fun mapIt(v: List<Int>): List<Int> = v.map { if (it > 0) 1 else 0 }

    fun withLocal(): Int {
        fun inner(a: Int): Int {
            return if (a > 0) 1 else 0
        }
        return inner(1)
    }

    fun chain(a: Int): Int {
        if (a == 1) {
            return 1
        } else if (a == 2) {
            return 2
        } else {
            return 3
        }
    }
}
"#;

#[test]
fn kotlin_when_counts_entries_but_not_else_and_is_one_cognitive_step() {
    let report = measured(Language::Kotlin, "Box.kt", KOTLIN);
    // `1, 2` (one entry), `3`; `else` adds nothing.
    assert_eq!(shape(&report, "Box.kind"), (3, 1, 1));
}

#[test]
fn kotlin_elvis_is_a_decision_with_no_cognitive_increment() {
    let report = measured(Language::Kotlin, "Box.kt", KOTLIN);
    assert_eq!(shape(&report, "Box.pick"), (2, 0, 0));
}

#[test]
fn kotlin_a_lambda_nests_inside_the_function_that_holds_it() {
    let report = measured(Language::Kotlin, "Box.kt", KOTLIN);
    // `if` inside the lambda: 1 + 1 nesting, plus `else` 1.
    assert_eq!(shape(&report, "Box.mapIt"), (2, 3, 1));
    assert_eq!(function(&report, "Box.mapIt").callees, [".map"]);
}

#[test]
fn kotlin_a_local_fun_is_its_own_unit() {
    let report = measured(Language::Kotlin, "Box.kt", KOTLIN);
    assert_eq!(shape(&report, "Box.withLocal"), (1, 0, 0));
    assert_eq!(function(&report, "Box.withLocal").callees, ["inner"]);
    assert_eq!(shape(&report, "Box.withLocal.inner"), (2, 2, 1));
}

#[test]
fn kotlin_a_secondary_constructor_is_a_function_of_its_class() {
    let report = measured(Language::Kotlin, "Box.kt", KOTLIN);
    assert_eq!(shape(&report, "Box.Box"), (1, 0, 0));
}

#[test]
fn kotlin_else_if_and_else_are_flat_increments() {
    let report = measured(Language::Kotlin, "Box.kt", KOTLIN);
    assert_eq!(shape(&report, "Box.chain"), (3, 3, 1));
}

const GO: &str = r#"
package shop

type Box struct{}

func (b *Box) Kind(x int) string {
	switch x {
	case 1, 2:
		return "a"
	case 3:
		return "b"
	default:
		return "c"
	}
}

func Pick(a, b chan int) int {
	select {
	case v := <-a:
		return v
	case v := <-b:
		return v
	default:
		return 0
	}
}

func TypeOf(v any) int {
	switch v.(type) {
	case int:
		return 1
	case string:
		return 2
	}
	return 0
}

func Jump(n int) int {
outer:
	for i := 0; i < n; i++ {
		for j := 0; j < n; j++ {
			if j == 1 {
				break outer
			}
		}
	}
	return n
}

func Lit() func() int {
	return func() int {
		if true {
			return 1
		}
		return 0
	}
}
"#;

#[test]
fn go_switch_counts_each_case_but_not_default_and_a_method_names_its_receiver() {
    let report = measured(Language::Go, "shop.go", GO);
    assert_eq!(shape(&report, "Box.Kind"), (3, 1, 1));
    // The receiver is not a parameter.
    assert_eq!(function(&report, "Box.Kind").size.parameters, 1);
}

#[test]
fn go_select_cases_are_branches_and_default_is_not() {
    let report = measured(Language::Go, "shop.go", GO);
    assert_eq!(shape(&report, "Pick"), (3, 1, 1));
    assert_eq!(function(&report, "Pick").size.parameters, 2);
}

#[test]
fn go_type_switch_cases_are_branches() {
    let report = measured(Language::Go, "shop.go", GO);
    assert_eq!(shape(&report, "TypeOf"), (3, 1, 1));
}

#[test]
fn go_a_labelled_break_costs_one_more() {
    let report = measured(Language::Go, "shop.go", GO);
    assert_eq!(shape(&report, "Jump"), (4, 7, 3));
}

#[test]
fn go_a_func_literal_belongs_to_the_function_that_holds_it() {
    let report = measured(Language::Go, "shop.go", GO);
    assert_eq!(shape(&report, "Lit"), (2, 2, 1));
    assert_eq!(report.functions.len(), 5);
}

#[test]
fn every_measurable_language_publishes_a_capability_matrix_that_does_not_overclaim() {
    for (language, path, source) in [
        (Language::Rust, "a.rs", RUST),
        (Language::Java, "A.java", JAVA),
        (Language::Kotlin, "A.kt", KOTLIN),
        (Language::Go, "a.go", GO),
    ] {
        let report = measured(language, path, source);
        assert_eq!(report.coverage, MetricsCoverage::Complete, "{path}");
        assert_eq!(report.capabilities.len(), MetricKind::ALL.len(), "{path}");
        assert!(
            report
                .capabilities
                .iter()
                .any(|c| c.metric == MetricKind::Cognitive
                    && matches!(c.support, MetricSupport::Partial { .. })),
            "{path}: cognitive complexity has known limits and must say so"
        );
    }
}

#[test]
fn a_language_without_rules_is_unsupported_not_zero() {
    for (path, source) in [("a.py", "def f():\n    return 1\n"), ("notes.txt", "hello")] {
        match measure_by_extension(path, source) {
            MetricsOutcome::Unsupported { reason, .. } => {
                assert!(reason.contains("nothing was measured"), "{reason}")
            }
            other => panic!("{path}: expected Unsupported, got {other:?}"),
        }
    }
}

#[test]
fn a_file_the_grammar_cannot_fully_read_is_partial_not_complete() {
    let report = measured(
        Language::Rust,
        "broken.rs",
        "fn fine() {}\nfn broken(a: i32 { if\n",
    );
    assert!(matches!(report.coverage, MetricsCoverage::Partial { .. }));
}

#[test]
fn the_same_source_gives_the_same_evidence_and_a_change_gives_another() {
    let first = measured(Language::Go, "shop.go", GO);
    let second = measured(Language::Go, "shop.go", GO);
    assert_eq!(first, second);
    assert_eq!(first.evidence_digest(), second.evidence_digest());

    let edited = GO.replace("case string:\n\t\treturn 2", "case string:\n\t\treturn 3");
    assert_ne!(edited, GO, "the edit must change the fixture");
    let third = measured(Language::Go, "shop.go", &edited);
    assert_ne!(first.evidence_digest(), third.evidence_digest());
}

#[test]
fn function_ids_are_stable_across_unrelated_edits_elsewhere_in_the_file() {
    let before = measured(Language::Rust, "src/lib.rs", RUST);
    let after = measured(
        Language::Rust,
        "src/lib.rs",
        &format!("{RUST}\nfn added() {{}}\n"),
    );
    for f in &before.functions {
        assert!(after.functions.iter().any(|g| g.id == f.id), "{}", f.id);
    }
}
