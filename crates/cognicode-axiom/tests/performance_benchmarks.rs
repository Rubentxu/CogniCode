//! Performance benchmarks for cognicode-axiom
//!
//! These tests use std::time to measure performance of core operations.
//! Run with: cargo test -p cognicode-axiom --test performance_benchmarks -- --nocapture

use std::path::PathBuf;
use std::time::Instant;

use cognicode_axiom::rules::{
    Category, DuplicationDetector, FileMetrics, Issue, ParseCache, ProjectMetrics,
    QualityGate, QualityGateEvaluator, RuleContext, RuleRegistry, Severity,
    TechnicalDebtCalculator,
};
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::parser::Language;

/// Simple inline Rust source for benchmarking (avoids external fixture dependency)
fn rust_source() -> &'static str {
    r#"pub struct UserService {
    name: String,
    email: String,
}

impl UserService {
    pub fn new(name: &str, email: &str) -> Self {
        Self {
            name: name.to_string(),
            email: email.to_string(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.email.contains('@') {
            Ok(())
        } else {
            Err("Invalid email".to_string())
        }
    }

    pub fn to_string(&self) -> String {
        format!("{} <{}>", self.name, self.email)
    }
}

fn main() {
    let user = UserService::new("Alice", "alice@example.com");
    println!("{}", user.to_string());
}
"#
}

#[test]
fn benchmark_parse_10k_lines() {
    let source = "fn test() {\n    let x = 1;\n}\n".repeat(3334); // ~10K lines
    let lang = Language::Rust;
    let start = Instant::now();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&lang.to_ts_language())
        .expect("Failed to set language");
    let tree = parser.parse(&source, None).expect("Parse failed");
    let elapsed = start.elapsed();
    println!(
        "Parse 10K lines: {:?} (target: <50ms, assert: <200ms)",
        elapsed
    );
    assert!(
        elapsed.as_millis() < 200,
        "Parse too slow: {}ms",
        elapsed.as_millis()
    );
    // Verify tree is valid
    assert!(tree.root_node().kind() == "source_file");
}

#[test]
fn benchmark_50_rules_parallel() {
    let source = rust_source();
    let registry = RuleRegistry::discover();
    let rules: Vec<_> = registry.for_language("Rust").into_iter().take(50).collect();
    let lang = Language::Rust;

    let start = Instant::now();
    // Parse once, run all rules
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&lang.to_ts_language())
        .expect("Failed to set language");
    let tree = parser.parse(source, None).expect("Parse failed");
    let graph = CallGraph::new();
    let metrics = FileMetrics::default();
    let file_path = PathBuf::from("test.rs");

    // Create context with proper lifetime - all data in same scope
    let ctx = RuleContext {
        tree: &tree,
        source,
        file_path: &file_path,
        language: &lang,
        graph: &graph,
        metrics: &metrics,
    };

    let mut total_issues = 0;
    for rule in &rules {
        total_issues += rule.check(&ctx).len();
    }
    let elapsed = start.elapsed();
    println!(
        "50 rules on {} lines: {:?} (target: <200ms, assert: <500ms), {} issues found",
        source.lines().count(),
        elapsed,
        total_issues
    );
    assert!(
        elapsed.as_millis() < 500,
        "50 rules too slow: {}ms",
        elapsed.as_millis()
    );
}

#[test]
fn benchmark_duplication_detection_100k() {
    let line = "    let x = 1;\n";
    let source = line.repeat(10000); // 10K lines with artificial duplication
    let detector = DuplicationDetector::new();
    let start = Instant::now();
    let groups = detector.detect_duplications(&source);
    let elapsed = start.elapsed();
    println!(
        "Duplication detection (10K lines): {:?} (target: <500ms, assert: <1s)",
        elapsed
    );
    println!("Found {} duplication groups", groups.len());
    assert!(
        elapsed.as_millis() < 1000,
        "Duplication too slow: {}ms",
        elapsed.as_millis()
    );
}

#[test]
fn benchmark_rule_registry_discovery() {
    let start = Instant::now();
    let registry = RuleRegistry::discover();
    let elapsed = start.elapsed();
    let rule_count = registry.all().len();
    println!(
        "RuleRegistry discovery ({} rules): {:?} (target: <10ms)",
        rule_count, elapsed
    );
    assert!(
        rule_count > 800,
        "Expected >800 rules, got {}",
        rule_count
    );
    assert!(
        elapsed.as_micros() < 10000,
        "Discovery too slow: {}µs",
        elapsed.as_micros()
    );
}

#[test]
fn benchmark_debt_calculation() {
    let issues: Vec<Issue> = (0..1000)
        .map(|i| {
            Issue::new(
                "S138",
                format!("Issue {}", i),
                Severity::Major,
                Category::CodeSmell,
                PathBuf::from("test.rs"),
                i % 100 + 1,
            )
        })
        .collect();
    let calc = TechnicalDebtCalculator::new();
    let start = Instant::now();
    let report = calc.calculate(&issues, 10000);
    let elapsed = start.elapsed();
    println!(
        "Debt calc (1000 issues): {:?} (target: <1ms, assert: <1ms), rating: {:?}",
        elapsed, report.rating
    );
    assert!(
        elapsed.as_micros() < 1000,
        "Debt calc too slow: {}µs",
        elapsed.as_micros()
    );
}

#[test]
fn benchmark_quality_gate_evaluation() {
    // Create a quality gate with some conditions
    let gate = QualityGate::new("Benchmark Gate", "Performance test gate")
        .add_condition(
            cognicode_axiom::rules::GateCondition::new(
                "code_smells",
                cognicode_axiom::rules::CompareOperator::LT,
                cognicode_axiom::rules::MetricValue::Integer(30),
            ),
        )
        .add_condition(
            cognicode_axiom::rules::GateCondition::new(
                "bugs",
                cognicode_axiom::rules::CompareOperator::LT,
                cognicode_axiom::rules::MetricValue::Integer(10),
            ),
        );

    let evaluator = QualityGateEvaluator::new(vec![gate]);
    let mut metrics = ProjectMetrics::new();
    metrics.code_smells = 25;
    metrics.bugs = 5;
    metrics.vulnerabilities = 0;

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = evaluator.evaluate_all(&metrics);
    }
    let elapsed = start.elapsed();
    let per_eval_ns = elapsed.as_nanos() / 1000;
    println!(
        "Quality gate eval (1000x): {:?} ({}ns per eval, target: <10µs)",
        elapsed, per_eval_ns
    );
    assert!(
        per_eval_ns < 10000,
        "Gate eval too slow: {}ns",
        per_eval_ns
    );
}

#[test]
fn benchmark_parse_cache_speedup() {
    let source = "fn test() {}\n".repeat(1000);
    let cache = ParseCache::new();

    // First, write a temp file for the cache to read
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("bench_test_rs_temp");
    std::fs::write(&temp_path, &source).expect("Failed to write temp file");

    // First parse (cold) - clear the cache first to ensure cold start
    cache.invalidate(&temp_path);
    let start = Instant::now();
    let _ = cache.get_or_parse(&temp_path);
    let cold = start.elapsed();

    // Second parse (cached)
    let start = Instant::now();
    let _ = cache.get_or_parse(&temp_path);
    let cached = start.elapsed();

    let speedup = cold.as_nanos() as f64 / cached.as_nanos().max(1) as f64;
    println!(
        "ParseCache: cold={:?}, cached={:?}, speedup={:.1}x (target: >10x, assert: >2x)",
        cold, cached, speedup
    );

    // Cleanup
    let _ = std::fs::remove_file(&temp_path);

    assert!(
        cached < cold,
        "Cache should be faster than cold parse"
    );
    assert!(
        speedup >= 2.0,
        "Expected speedup >= 2x, got {:.1}x",
        speedup
    );
}
