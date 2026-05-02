//! Criterion benchmarks for cognicode-axiom rule engine
//!
//! Run with: cargo bench -p cognicode-axiom --bench rule_benchmarks

use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use cognicode_axiom::rules::{
    Category, CompareOperator, DuplicationDetector, FileMetrics, GateCondition, Issue,
    MetricValue, ParseCache, ProjectMetrics, QualityGate, QualityGateEvaluator, RuleContext,
    RuleRegistry, Severity, TechnicalDebtCalculator,
};
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::infrastructure::parser::Language;

/// Simple inline Rust source for benchmarking
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

fn benchmark_parse_10k_lines(c: &mut Criterion) {
    let source = "fn test() {\n    let x = 1;\n}\n".repeat(3334); // ~10K lines
    let lang = Language::Rust;

    c.bench_function("parse_10k_lines", |b| {
        b.iter(|| {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&lang.to_ts_language())
                .expect("Failed to set language");
            let _tree = parser.parse(black_box(&source), None).expect("Parse failed");
        });
    });
}

fn benchmark_50_rules_parallel(c: &mut Criterion) {
    let source = rust_source();
    let registry = RuleRegistry::discover();
    let rules: Vec<_> = registry.for_language("Rust").into_iter().take(50).collect();
    let lang = Language::Rust;

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

    c.bench_function("50_rules_parallel", |b| {
        b.iter(|| {
            let mut total_issues = 0;
            for rule in &rules {
                total_issues += rule.check(black_box(&ctx)).len();
            }
            total_issues
        });
    });
}

fn benchmark_duplication_detection(c: &mut Criterion) {
    let line = "    let x = 1;\n";
    let source = line.repeat(10000); // 10K lines with artificial duplication
    let detector = DuplicationDetector::new();

    c.bench_function("duplication_detection_10k_lines", |b| {
        b.iter(|| {
            let _groups = detector.detect_duplications(black_box(&source));
        });
    });
}

fn benchmark_rule_registry_discovery(c: &mut Criterion) {
    c.bench_function("rule_registry_discovery", |b| {
        b.iter(|| {
            let registry = RuleRegistry::discover();
            registry.all().len()
        });
    });
}

fn benchmark_debt_calculation(c: &mut Criterion) {
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

    c.bench_function("debt_calculation_1000_issues", |b| {
        b.iter(|| {
            let _report = calc.calculate(black_box(&issues), 10000);
        });
    });
}

fn benchmark_quality_gate_evaluation(c: &mut Criterion) {
    let gate = QualityGate::new("Benchmark Gate", "Performance test gate")
        .add_condition(GateCondition::new(
            "code_smells",
            CompareOperator::LT,
            MetricValue::Integer(30),
        ))
        .add_condition(GateCondition::new(
            "bugs",
            CompareOperator::LT,
            MetricValue::Integer(10),
        ))
        .add_condition(GateCondition::new(
            "vulnerabilities",
            CompareOperator::EQ,
            MetricValue::Integer(0),
        ));

    let evaluator = QualityGateEvaluator::new(vec![gate]);
    let mut metrics = ProjectMetrics::new();
    metrics.code_smells = 25;
    metrics.bugs = 5;
    metrics.vulnerabilities = 0;

    c.bench_function("quality_gate_evaluation", |b| {
        b.iter(|| {
            let _ = evaluator.evaluate_all(black_box(&metrics));
        });
    });
}

fn benchmark_parse_cache(c: &mut Criterion) {
    let source = "fn test() {}\n".repeat(1000);
    let cache = ParseCache::new();

    // Create temp file for cache
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join("criterion_bench_test_rs");
    std::fs::write(&temp_path, &source).expect("Failed to write temp file");

    // Cold parse
    cache.invalidate(&temp_path);
    let _ = cache.get_or_parse(&temp_path);

    c.bench_function("parse_cache_cached", |b| {
        b.iter(|| {
            let _ = cache.get_or_parse(black_box(&temp_path));
        });
    });

    // Cleanup
    let _ = std::fs::remove_file(&temp_path);
}

criterion_group!(
    benches,
    benchmark_parse_10k_lines,
    benchmark_50_rules_parallel,
    benchmark_duplication_detection,
    benchmark_rule_registry_discovery,
    benchmark_debt_calculation,
    benchmark_quality_gate_evaluation,
    benchmark_parse_cache,
);
criterion_main!(benches);
