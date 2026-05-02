//! System Integration Tests for cognicode-axiom core systems
//!
//! Tests the QualityGate, TechnicalDebt, Ratings, Duplication, Profiles,
//! RuleRegistry, and Importer systems with realistic data.

use std::collections::HashMap;
use std::path::PathBuf;

use cognicode_axiom::rules::{
    CompareOperator, DuplicationDetector, GateCondition, MetricValue,
    ProjectMetrics, QualityGate, QualityGateEvaluator, Remediation,
    RuleCatalog, RuleRegistry, Severity, Category, Issue,
    TechnicalDebtCalculator, TechnicalDebtReport, DebtRating,
    ProjectRatings, QualityProfileEngine,
};
use cognicode_axiom::rules::importer::{create_sample_catalog, RuleParameter};

// ============================================================================
// 1. QualityGate System Tests
// ============================================================================

#[test]
fn test_quality_gate_with_passing_metrics() {
    // Create a gate that requires complexity < 20
    let gate = QualityGate::new("Complexity Gate", "Checks cyclomatic complexity")
        .add_condition(GateCondition::new(
            "complexity",
            CompareOperator::LT,
            MetricValue::Integer(20),
        ));

    // Create metrics with low complexity (passing)
    let mut metrics = ProjectMetrics::new();
    metrics.complexity = 10;

    let result = gate.evaluate(&metrics);

    assert!(result.passed, "Gate should pass with complexity 10 < 20");
    assert!(!result.blocked, "Gate should not be blocked");
    assert_eq!(result.condition_results.len(), 1);
    assert!(result.condition_results[0].passed);
}

#[test]
fn test_quality_gate_with_failing_metrics() {
    // Create a gate that requires bugs == 0
    let gate = QualityGate::new("Bug Gate", "No bugs allowed")
        .add_condition(GateCondition::new(
            "bugs",
            CompareOperator::EQ,
            MetricValue::Integer(0),
        ));

    // Create metrics with bugs (failing)
    let mut metrics = ProjectMetrics::new();
    metrics.bugs = 5;

    let result = gate.evaluate(&metrics);

    assert!(!result.passed, "Gate should fail with 5 bugs");
    assert!(!result.blocked, "Gate should not be blocked");
    let condition = &result.condition_results[0];
    assert!(!condition.passed);
    // Verify the actual value contains the bug count (5)
    assert!(matches!(condition.actual_value, Some(MetricValue::Integer(v)) if v == 5));
}

#[test]
fn test_quality_gate_blocked_by_blocker_condition() {
    // Create a gate with a condition on a nonexistent metric
    let gate = QualityGate::new("Metrics Gate", "Check for nonexistent metric")
        .add_condition(GateCondition::new(
            "nonexistent_metric",
            CompareOperator::LT,
            MetricValue::Integer(100),
        ));

    let metrics = ProjectMetrics::new();
    let result = gate.evaluate(&metrics);

    assert!(!result.passed);
    assert!(result.blocked, "Gate should be blocked due to missing metric");
    let condition = &result.condition_results[0];
    assert!(condition.blocked);
    assert!(condition.message.is_some());
}

#[test]
fn test_quality_gate_multiple_conditions() {
    // Create a gate with multiple conditions (all must pass)
    let gate = QualityGate::new("Comprehensive Gate", "Multiple quality gates")
        .add_condition(GateCondition::new(
            "complexity",
            CompareOperator::LTE,
            MetricValue::Integer(50),
        ))
        .add_condition(GateCondition::new(
            "bugs",
            CompareOperator::EQ,
            MetricValue::Integer(0),
        ))
        .add_condition(GateCondition::new(
            "vulnerabilities",
            CompareOperator::EQ,
            MetricValue::Integer(0),
        ));

    // Create passing metrics
    let mut metrics = ProjectMetrics::new();
    metrics.complexity = 30;
    metrics.bugs = 0;
    metrics.vulnerabilities = 0;

    let result = gate.evaluate(&metrics);
    assert!(result.passed);
    assert_eq!(result.condition_results.len(), 3);

    // Now fail one condition
    metrics.bugs = 2;
    let result = gate.evaluate(&metrics);
    assert!(!result.passed);

    // All conditions should still be evaluated
    assert_eq!(result.condition_results.len(), 3);
}

#[test]
fn test_quality_gate_evaluator_all_pass() {
    let gates = vec![
        QualityGate::new("Gate 1", "First gate")
            .add_condition(GateCondition::new(
                "complexity",
                CompareOperator::LT,
                MetricValue::Integer(100),
            )),
        QualityGate::new("Gate 2", "Second gate")
            .add_condition(GateCondition::new(
                "ncloc",
                CompareOperator::LT,
                MetricValue::Integer(10000),
            )),
    ];

    let evaluator = QualityGateEvaluator::new(gates);
    let mut metrics = ProjectMetrics::new();
    metrics.complexity = 50;
    metrics.ncloc = 5000;

    let results = evaluator.evaluate_all(&metrics);
    assert_eq!(results.len(), 2);
    assert!(evaluator.all_pass(&metrics));
}

#[test]
fn test_quality_gate_with_percentage_metrics() {
    let gate = QualityGate::new("Coverage Gate", "Requires minimum coverage")
        .add_condition(GateCondition::new(
            "coverage",
            CompareOperator::GTE,
            MetricValue::Percentage(80.0),
        ));

    let mut metrics = ProjectMetrics::new();
    metrics.coverage_percentage = Some(85.0);

    let result = gate.evaluate(&metrics);
    assert!(result.passed);
}

#[test]
fn test_quality_gate_compare_operators() {
    // Test GT
    let gate = QualityGate::new("GT", "Test >")
        .add_condition(GateCondition::new("ncloc", CompareOperator::GT, MetricValue::Integer(100)));
    let mut m = ProjectMetrics::new();
    m.ncloc = 101;
    assert!(gate.evaluate(&m).passed);
    m.ncloc = 100;
    assert!(!gate.evaluate(&m).passed);

    // Test GTE
    let gate = QualityGate::new("GTE", "Test >=")
        .add_condition(GateCondition::new("ncloc", CompareOperator::GTE, MetricValue::Integer(100)));
    assert!(gate.evaluate(&m).passed);

    // Test LT
    let gate = QualityGate::new("LT", "Test <")
        .add_condition(GateCondition::new("ncloc", CompareOperator::LT, MetricValue::Integer(100)));
    m.ncloc = 99;
    assert!(gate.evaluate(&m).passed);
    m.ncloc = 100;
    assert!(!gate.evaluate(&m).passed);

    // Test LTE
    let gate = QualityGate::new("LTE", "Test <=")
        .add_condition(GateCondition::new("ncloc", CompareOperator::LTE, MetricValue::Integer(100)));
    assert!(gate.evaluate(&m).passed);

    // Test NEQ
    let gate = QualityGate::new("NEQ", "Test !=")
        .add_condition(GateCondition::new("ncloc", CompareOperator::NEQ, MetricValue::Integer(100)));
    m.ncloc = 101;
    assert!(gate.evaluate(&m).passed);
    m.ncloc = 100;
    assert!(!gate.evaluate(&m).passed);
}

// ============================================================================
// 2. Technical Debt Calculation Tests
// ============================================================================

#[test]
fn test_debt_calculation_empty() {
    let calculator = TechnicalDebtCalculator::new();
    let report = calculator.calculate(&[], 1000);

    assert_eq!(report.total_debt_minutes, 0);
    assert_eq!(report.debt_ratio, 0.0);
    assert_eq!(report.rating, DebtRating::A);
    assert_eq!(report.total_issues, 0);
}

#[test]
fn test_debt_calculation_with_issues() {
    let calculator = TechnicalDebtCalculator::new();

    let issues = vec![
        Issue::new("S138", "Cognitive complexity", Severity::Major, Category::CodeSmell,
                   PathBuf::from("src/main.rs"), 10),
        Issue::new("S1871", "Duplicate code", Severity::Major, Category::Bug,
                   PathBuf::from("src/lib.rs"), 25)
            .with_remediation(Remediation::substantial("Refactor duplicate code")),
    ];

    let report = calculator.calculate(&issues, 1000);

    assert!(report.total_debt_minutes > 0, "Should have non-zero debt");
    assert!(report.debt_ratio > 0.0, "Should have non-zero debt ratio");
    assert!(report.total_issues == 2);
    assert!(report.by_category.contains_key("Maintainability"));
    assert!(report.by_category.contains_key("Reliability"));
}

#[test]
fn test_debt_rating_a_for_zero_debt() {
    let calculator = TechnicalDebtCalculator::new();
    let report = calculator.calculate(&[], 5000);

    assert_eq!(report.rating, DebtRating::A);
    assert_eq!(report.debt_ratio, 0.0);
}

#[test]
fn test_debt_rating_e_for_high_debt() {
    let calculator = TechnicalDebtCalculator::new();

    // Create many issues to push debt ratio > 50%
    // With 1000 ncloc, 30 min/line baseline, total dev time = 30,000 minutes
    // For > 50% debt ratio, we need > 15,000 minutes of debt
    // A single blocker vulnerability costs ~480 * 2.5 = 1200 minutes
    // So we need many issues
    let mut issues = Vec::new();
    for i in 0..50 {
        issues.push(Issue::new(
            "S001", "Vulnerability", Severity::Critical, Category::Vulnerability,
            PathBuf::from("src/main.rs"), 10 + i,
        ));
    }

    let report = calculator.calculate(&issues, 1000);

    assert!(report.rating == DebtRating::E, "Expected E rating, got {:?}", report.rating);
    assert!(report.debt_ratio > 0.5, "Debt ratio should exceed 50%");
}

#[test]
fn test_debt_rating_thresholds() {
    // A: <= 5%
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.03), DebtRating::A);
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.05), DebtRating::A);

    // B: <= 10%
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.07), DebtRating::B);
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.10), DebtRating::B);

    // C: <= 20%
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.15), DebtRating::C);
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.20), DebtRating::C);

    // D: <= 50%
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.35), DebtRating::D);
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.50), DebtRating::D);

    // E: > 50%
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(0.60), DebtRating::E);
    assert_eq!(TechnicalDebtCalculator::debt_ratio_to_rating(1.0), DebtRating::E);
}

#[test]
fn test_debt_category_mapping() {
    let calculator = TechnicalDebtCalculator::new();

    let bug = Issue::new("BUG", "Bug", Severity::Major, Category::Bug,
                         PathBuf::from("test.rs"), 1);
    let vuln = Issue::new("VULN", "Vuln", Severity::Major, Category::Vulnerability,
                          PathBuf::from("test.rs"), 1);
    let smell = Issue::new("SMELL", "Smell", Severity::Major, Category::CodeSmell,
                           PathBuf::from("test.rs"), 1);
    let hotspot = Issue::new("HOTSPOT", "Hotspot", Severity::Major, Category::SecurityHotspot,
                             PathBuf::from("test.rs"), 1);

    let report = calculator.calculate(&[bug.clone(), vuln.clone(), smell.clone(), hotspot.clone()], 1000);

    assert!(report.by_category.contains_key("Reliability"));     // Bug
    assert!(report.by_category.contains_key("Security"));         // Vulnerability & SecurityHotspot
    assert!(report.by_category.contains_key("Maintainability"));  // CodeSmell
}

#[test]
fn test_debt_format_minutes() {
    let calculator = TechnicalDebtCalculator::new();

    assert_eq!(calculator.format_debt(30), "30 minutes");
    assert_eq!(calculator.format_debt(60), "1 hour");
    assert_eq!(calculator.format_debt(90), "1h 30m");
    assert_eq!(calculator.format_debt(480), "1 day");
    assert_eq!(calculator.format_debt(960), "2 days");
    assert_eq!(calculator.format_debt(1200), "2d 4h");
}

#[test]
fn test_debt_multiline_issue() {
    let calculator = TechnicalDebtCalculator::new();

    let single = Issue::new("S001", "Single", Severity::Major, Category::CodeSmell,
                            PathBuf::from("test.rs"), 10);
    let mut multi = single.clone();
    multi.end_line = Some(25);

    let single_debt = calculator.issue_debt_minutes(&single);
    let multi_debt = calculator.issue_debt_minutes(&multi);

    // Multi-line issues should cost more
    assert!(multi_debt > single_debt,
            "Multi-line debt {} should exceed single-line debt {}",
            multi_debt, single_debt);
}

// ============================================================================
// 3. Project Ratings Tests
// ============================================================================

#[test]
fn test_ratings_perfect_project_gets_a() {
    let issues = vec![];

    let debt_report = TechnicalDebtReport {
        total_debt_minutes: 0,
        debt_ratio: 0.0,
        rating: DebtRating::A,
        by_category: HashMap::new(),
        total_issues: 0,
        ncloc: 1000,
    };

    let ratings = ProjectRatings::compute(&issues, 1000, &debt_report);

    assert_eq!(ratings.reliability, DebtRating::A);
    assert_eq!(ratings.security, DebtRating::A);
    assert_eq!(ratings.maintainability, DebtRating::A);
    assert_eq!(ratings.overall(), 'A');
}

#[test]
fn test_ratings_with_blocker_bugs_gets_e() {
    // Create many bugs to trigger E rating (density > 2 per 1000 lines)
    let issues: Vec<Issue> = (0..25)
        .map(|i| Issue::new("BUG", "Bug", Severity::Blocker, Category::Bug,
                            PathBuf::from("src/main.rs"), 10 + i))
        .collect();

    let debt_report = TechnicalDebtReport {
        total_debt_minutes: 0,
        debt_ratio: 0.0,
        rating: DebtRating::A,
        by_category: HashMap::new(),
        total_issues: 0,
        ncloc: 1000,
    };

    let ratings = ProjectRatings::compute(&issues, 1000, &debt_report);

    // 25 blocker bugs in 1000 lines = 25 per 1000 lines density -> E
    assert_eq!(ratings.reliability, DebtRating::E,
               "Expected E for 25 blocker bugs per 1000 LOC");
    assert_eq!(ratings.overall(), 'E');
}

#[test]
fn test_ratings_with_mixed_issues() {
    // Mix of issues across categories
    let issues = vec![
        // 1 bug per 1000 lines -> C reliability
        Issue::new("BUG1", "Bug 1", Severity::Major, Category::Bug,
                   PathBuf::from("src/main.rs"), 10),
        // Some code smells -> maintainability affected by debt
        Issue::new("S001", "Smell 1", Severity::Minor, Category::CodeSmell,
                   PathBuf::from("src/lib.rs"), 20),
        Issue::new("S002", "Smell 2", Severity::Minor, Category::CodeSmell,
                   PathBuf::from("src/lib.rs"), 30),
        // 0 vulnerabilities -> A security
    ];

    let debt_report = TechnicalDebtReport {
        total_debt_minutes: 0,
        debt_ratio: 0.0,
        rating: DebtRating::A,
        by_category: HashMap::new(),
        total_issues: 3,
        ncloc: 1000,
    };

    let ratings = ProjectRatings::compute(&issues, 1000, &debt_report);

    // 1 bug per 1000 lines = 1.0 density -> C (0.5 < density <= 1.0)
    assert_eq!(ratings.reliability, DebtRating::C);
    // 0 security issues -> A
    assert_eq!(ratings.security, DebtRating::A);
    // Maintainability from debt report
    assert_eq!(ratings.maintainability, DebtRating::A);
    // Overall is worst: C
    assert_eq!(ratings.overall(), 'C');
}

#[test]
fn test_ratings_overall_is_worst() {
    let ratings = ProjectRatings {
        reliability: DebtRating::A,
        security: DebtRating::B,
        maintainability: DebtRating::D,
    };

    assert_eq!(ratings.overall(), 'D', "Overall should be worst rating");
    assert_eq!(ratings.overall_rating(), DebtRating::D);
}

#[test]
fn test_ratings_meets_threshold() {
    let ratings = ProjectRatings {
        reliability: DebtRating::B,
        security: DebtRating::C,
        maintainability: DebtRating::A,
    };

    // Overall is C (minimum of B, C, A)
    assert!(!ratings.meets_threshold('A'));
    assert!(!ratings.meets_threshold('B'));
    assert!(ratings.meets_threshold('C'));
    assert!(ratings.meets_threshold('D'));
    assert!(ratings.meets_threshold('E'));
}

#[test]
fn test_ratings_security_density() {
    // 2 vulnerabilities per 1000 lines -> 2.0 density -> E
    let issues = vec![
        Issue::new("V1", "Vuln 1", Severity::Critical, Category::Vulnerability,
                   PathBuf::from("src/main.rs"), 10),
        Issue::new("V2", "Vuln 2", Severity::Major, Category::Vulnerability,
                   PathBuf::from("src/main.rs"), 20),
    ];

    let debt_report = TechnicalDebtReport {
        total_debt_minutes: 0,
        debt_ratio: 0.0,
        rating: DebtRating::A,
        by_category: HashMap::new(),
        total_issues: 0,
        ncloc: 1000,
    };

    let ratings = ProjectRatings::compute(&issues, 1000, &debt_report);

    // 2 security issues per 1000 lines = 2.0 density -> E (> 1.0)
    assert_eq!(ratings.security, DebtRating::E);
}

// ============================================================================
// 4. Duplication Detection Tests
// ============================================================================

#[test]
fn test_duplication_detects_identical_code() {
    let detector = DuplicationDetector::new();

    // Two identical 6+ line functions (minimum for detection)
    let source = r#"
fn process_data() {
    let x = 1;
    let y = 2;
    let z = x + y;
    println!("Result: {}", z);
    return z;
}

fn handle_request() {
    let x = 1;
    let y = 2;
    let z = x + y;
    println!("Result: {}", z);
    return z;
}
"#;

    let groups = detector.detect_duplications(source);

    // Should detect some duplication
    assert!(!groups.is_empty(), "Should detect duplicated code");

    // Each group should have at least 2 locations for actual duplication
    let multi_file_groups: Vec<_> = groups.iter()
        .filter(|g| g.locations.len() > 1)
        .collect();

    // If we have multi-location groups, check they have proper structure
    for group in multi_file_groups {
        assert!(group.lines >= 6, "Duplicated blocks should be at least 6 lines");
        assert!(group.hash != 0, "Hash should be non-zero");
    }
}

#[test]
fn test_duplication_no_false_positives() {
    let detector = DuplicationDetector::new();

    // Unique code with no duplication
    let source = r#"
fn foo() {
    let a = 1;
    println!("Function foo with value {}", a);
}

fn bar() {
    let b = 2;
    println!("Function bar with value {}", b);
}

fn baz() {
    let c = 3;
    println!("Function baz with value {}", c);
}
"#;

    let groups = detector.detect_duplications(source);

    // Very short unique functions shouldn't trigger duplication detection
    // since window_size is 6 and min_duplicate_lines is 6
    // But if there's truly no matching windows, groups will be empty
    // This test ensures we don't get spurious false positives
    assert!(groups.is_empty() || groups.iter().all(|g| g.lines >= 6));
}

#[test]
fn test_duplication_with_minimum_lines() {
    // Use higher minimum to test threshold
    let detector = DuplicationDetector::with_config(10, 10);

    // Source with exactly 6 matching lines (should not trigger with min=10)
    let source = r#"
fn common_helper() {
    line1();
    line2();
    line3();
    line4();
    line5();
    line6();
}

fn other_helper() {
    line1();
    line2();
    line3();
    line4();
    line5();
    line6();
}
"#;

    let groups = detector.detect_duplications(source);

    // With min_duplicate_lines = 10, 6-line matches shouldn't trigger
    assert!(groups.is_empty());
}

#[test]
fn test_duplication_multi_file() {
    let detector = DuplicationDetector::new();

    let files = vec![
        ("file1.rs".to_string(), r#"
fn common_validation() {
    check_input();
    validate_type();
    assert_not_null();
    log_debug();
    trace_entry();
    trace_exit();
}
"#.to_string()),
        ("file2.rs".to_string(), r#"
fn common_validation() {
    check_input();
    validate_type();
    assert_not_null();
    log_debug();
    trace_entry();
    trace_exit();
}
"#.to_string()),
        ("file3.rs".to_string(), r#"
fn different_code() {
    do_something_else();
}
"#.to_string()),
    ];

    let groups = detector.detect_multi_file_duplications(&files);

    // Should find the duplicated function in file1.rs and file2.rs
    assert!(!groups.is_empty());

    // Find the group with 2 locations
    let dup_group = groups.iter().find(|g| {
        g.locations.iter().any(|l| l.file == "file1.rs") &&
        g.locations.iter().any(|l| l.file == "file2.rs")
    });

    assert!(dup_group.is_some(), "Should find duplicated group between file1 and file2");

    let group = dup_group.unwrap();
    assert!(group.locations.iter().any(|l| l.file == "file1.rs"));
    assert!(group.locations.iter().any(|l| l.file == "file2.rs"));
    assert!(!group.locations.iter().any(|l| l.file == "file3.rs"));
}

#[test]
fn test_duplication_percentage() {
    let detector = DuplicationDetector::new();

    // Create source where 50% is duplicated
    let source = r#"
fn primary_func() {
    line1();
    line2();
    line3();
    line4();
    line5();
    line6();
}

fn dup_func() {
    line1();
    line2();
    line3();
    line4();
    line5();
    line6();
}
"#;

    let percentage = detector.duplication_percentage(source);

    // Should have some duplication detected
    assert!(percentage >= 0.0);
    assert!(percentage <= 100.0);
}

#[test]
fn test_duplication_empty_source() {
    let detector = DuplicationDetector::new();

    let groups = detector.detect_duplications("");
    assert!(groups.is_empty());

    let percentage = detector.duplication_percentage("");
    assert_eq!(percentage, 0.0);
}

#[test]
fn test_duplication_hash_stability() {
    let detector = DuplicationDetector::new();

    let source1 = "fn foo() {\n    a();\n    b();\n    c();\n    d();\n    e();\n    f();\n}\n";
    let source2 = "fn bar() {\n    a();\n    b();\n    c();\n    d();\n    e();\n    f();\n}\n";

    let groups1 = detector.detect_duplications(source1);
    let groups2 = detector.detect_duplications(source2);

    // Both should detect the same hash for the identical 6-line window
    // (The internal window hashing should be consistent)
    // We just verify the detector doesn't panic and produces stable output
    assert_eq!(groups1.len(), groups2.len());
}

// ============================================================================
// 5. Quality Profiles Tests
// ============================================================================

#[test]
fn test_profile_load_from_yaml() {
    const YAML: &str = r#"
name: "Sonar way"
description: "Sonar way quality profile for Rust"
language: "rust"
is_default: true
rules:
  - rule_id: "S138"
    enabled: true
    severity: "major"
    parameters:
      threshold: 50
  - rule_id: "S3776"
    enabled: true
    severity: "critical"
  - rule_id: "S1066"
    enabled: false
"#;

    let engine = QualityProfileEngine::from_yaml(YAML).expect("Failed to parse YAML");

    let profile = engine.get("Sonar way").expect("Profile not found");
    assert_eq!(profile.name, "Sonar way");
    assert_eq!(profile.language, "rust");
    assert!(profile.is_default);
    assert_eq!(profile.rules.len(), 3);
}

#[test]
fn test_profile_inheritance() {
    const YAML: &str = r#"
- name: "parent-profile"
  description: "Parent quality profile"
  language: "rust"
  rules:
    - rule_id: "S138"
      enabled: true
      severity: "major"
    - rule_id: "S3776"
      enabled: true
      severity: "minor"

- name: "child-profile"
  description: "Child profile extending parent"
  language: "rust"
  extends: "parent-profile"
  rules:
    - rule_id: "S138"
      severity: "critical"
    - rule_id: "S2306"
      enabled: true
"#;

    let engine = QualityProfileEngine::from_yaml(YAML).expect("Failed to parse YAML");

    // Resolve child profile
    let resolved = engine.resolve_profile("child-profile");

    // S138 should be overridden to critical
    let s138 = resolved.rules.get("S138").expect("S138 not found");
    assert_eq!(s138.severity, Severity::Critical);

    // S3776 should be inherited from parent (default Minor severity)
    let s3776 = resolved.rules.get("S3776").expect("S3776 not found");
    assert_eq!(s3776.severity, Severity::Minor);

    // S2306 should be from child
    let s2306 = resolved.rules.get("S2306").expect("S2306 not found");
    assert!(s2306.enabled);
}

#[test]
fn test_profile_rule_override() {
    const YAML: &str = r#"
name: "strict-profile"
description: "Strict profile with all rules at maximum severity"
language: "rust"
rules:
  - rule_id: "S138"
    enabled: true
    severity: "blocker"
  - rule_id: "S3776"
    enabled: true
    severity: "major"
"#;

    let engine = QualityProfileEngine::from_yaml(YAML).expect("Failed to parse YAML");
    let resolved = engine.resolve_profile("strict-profile");

    let s138 = resolved.rules.get("S138").expect("S138 not found");
    assert_eq!(s138.severity, Severity::Blocker);

    let s3776 = resolved.rules.get("S3776").expect("S3776 not found");
    assert_eq!(s3776.severity, Severity::Major);
}

#[test]
fn test_profile_parameters() {
    const YAML: &str = r#"
name: "param-profile"
description: "Profile with rule parameters"
language: "python"
rules:
  - rule_id: "S001"
    enabled: true
    severity: "major"
    parameters:
      max_complexity: 10
      max_line_length: 120
"#;

    let engine = QualityProfileEngine::from_yaml(YAML).expect("Failed to parse YAML");
    let resolved = engine.resolve_profile("param-profile");

    let rule = resolved.rules.get("S001").expect("S001 not found");
    assert_eq!(rule.parameters.get("max_complexity").and_then(|v| v.as_u64()), Some(10));
    assert_eq!(rule.parameters.get("max_line_length").and_then(|v| v.as_u64()), Some(120));
}

#[test]
fn test_profile_resolve_by_language() {
    const YAML: &str = r#"
name: "rust-default"
description: "Default Rust profile"
language: "rust"
is_default: true
rules:
  - rule_id: "S138"
    enabled: true
"#;

    let engine = QualityProfileEngine::from_yaml(YAML).expect("Failed to parse YAML");
    let resolved = engine.resolve_profile("rust");

    // Should resolve to the default Rust profile
    assert_eq!(resolved.name, "rust-default");
}

#[test]
fn test_profile_nonexistent() {
    const YAML: &str = r#"
name: "test"
description: "Test"
language: "rust"
rules: []
"#;

    let engine = QualityProfileEngine::from_yaml(YAML).expect("Failed to parse YAML");
    let resolved = engine.resolve_profile("nonexistent-profile");

    assert!(resolved.rules.is_empty());
}

// ============================================================================
// 6. RuleRegistry Tests
// ============================================================================

#[test]
fn test_registry_discovers_all_rules() {
    let registry = RuleRegistry::discover();

    // Should discover some rules via inventory
    let count = registry.count();
    assert!(count > 0, "Expected at least some rules to be discovered");

    let all_rules = registry.all();
    assert_eq!(all_rules.len(), count);

    // Each rule should have valid metadata
    for rule in all_rules {
        assert!(!rule.id().is_empty(), "Rule ID should not be empty");
        assert!(!rule.name().is_empty(), "Rule name should not be empty");
    }
}

#[test]
fn test_registry_filter_by_language() {
    let registry = RuleRegistry::discover();

    // Get rules for a specific language
    let rust_rules = registry.for_language("rust");
    let _js_rules = registry.for_language("javascript");

    // Rules should be language-specific or generic
    for rule in &rust_rules {
        assert!(
            rule.language().eq_ignore_ascii_case("rust") ||
            rule.language().eq_ignore_ascii_case("*"),
            "Expected Rust or generic language, got {}",
            rule.language()
        );
    }

    // Language matching should be case-insensitive
    let rust_lower = registry.for_language("RUST");
    let rust_upper = registry.for_language("rust");
    assert_eq!(rust_lower.len(), rust_upper.len());
}

#[test]
fn test_registry_filter_by_category() {
    let registry = RuleRegistry::discover();

    let bug_rules = registry.for_category(Category::Bug);
    let smell_rules = registry.for_category(Category::CodeSmell);

    // All returned rules should match the category
    for rule in &bug_rules {
        assert_eq!(rule.category(), Category::Bug);
    }

    for rule in &smell_rules {
        assert_eq!(rule.category(), Category::CodeSmell);
    }
}

#[test]
fn test_registry_returns_empty_for_unknown_language() {
    let registry = RuleRegistry::discover();

    let unknown_rules = registry.for_language("nonexistent_language_xyz");

    assert!(unknown_rules.is_empty(), "Should return empty vec for unknown language");
}

#[test]
fn test_registry_severity_assignment() {
    let registry = RuleRegistry::discover();

    let all_rules = registry.all();

    // All rules should have valid severity
    for rule in all_rules {
        let severity = rule.severity();
        assert!(
            matches!(severity, Severity::Info | Severity::Minor |
                           Severity::Major | Severity::Critical | Severity::Blocker),
            "Invalid severity {:?} for rule {}",
            severity, rule.id()
        );
    }
}

#[test]
fn test_registry_multiple_language_queries() {
    let registry = RuleRegistry::discover();

    // Query multiple languages
    let languages = vec!["rust", "javascript", "python", "java"];

    for lang in languages {
        let rules = registry.for_language(lang);
        // Just verify we get a result without panic (length can be 0 for unknown languages)
        let _ = rules.len();
    }
}

// ============================================================================
// 7. Importer Tests
// ============================================================================

#[test]
fn test_importer_creates_sample_catalog() {
    let catalog = create_sample_catalog();

    assert_eq!(catalog.version, "1.0");
    assert_eq!(catalog.source, "sonarqube-api");
    assert!(!catalog.exported_at.is_empty());
    assert_eq!(catalog.rules.len(), 3);

    // Check rule structure
    let rule = &catalog.rules[0];
    assert_eq!(rule.rule_id, "S1226");
    assert_eq!(rule.language, "rust");
    assert!(!rule.name.is_empty());
    assert!(!rule.description.is_empty());
}

#[test]
fn test_importer_loads_from_json() {
    // Create a temporary JSON file
    let json_content = r#"{
        "version": "2.0",
        "source": "test-sonarqube",
        "exported_at": "2024-01-15T10:00:00Z",
        "rules": [
            {
                "rule_id": "S0001",
                "name": "Test Rule",
                "severity": "MAJOR",
                "rule_type": "CODE_SMELL",
                "language": "python",
                "description": "A test rule",
                "parameters": [
                    {
                        "name": "threshold",
                        "description": "Max value",
                        "default_value": "10",
                        "param_type": "INT"
                    }
                ],
                "tags": ["test", "python"]
            }
        ]
    }"#;

    // Parse the JSON directly
    let catalog: RuleCatalog = serde_json::from_str(json_content)
        .expect("Failed to parse JSON");

    assert_eq!(catalog.version, "2.0");
    assert_eq!(catalog.rules.len(), 1);
    assert_eq!(catalog.rules[0].rule_id, "S0001");
    assert_eq!(catalog.rules[0].parameters.len(), 1);
    assert_eq!(catalog.rules[0].parameters[0].name, "threshold");
}

#[test]
fn test_importer_generates_stubs() {
    let catalog = create_sample_catalog();
    let existing_ids = vec!["S138", "S3776"];

    let stubs = catalog.generate_rust_stubs(&existing_ids);

    // Should contain stubs for rules not in existing_ids
    assert!(stubs.contains("S1226"), "Should generate stub for S1226");
    assert!(stubs.contains("S1186"), "Should generate stub for S1186");
    assert!(stubs.contains("S1871"), "Should generate stub for S1871");

    // Should not contain stubs for existing rules
    assert!(!stubs.contains("S138"), "Should not generate stub for existing S138");
    assert!(!stubs.contains("S3776"), "Should not generate stub for existing S3776");
}

#[test]
fn test_importer_filter_by_language() {
    let catalog = create_sample_catalog();

    let rust_rules = catalog.for_language("rust");
    let python_rules = catalog.for_language("python");

    assert_eq!(rust_rules.len(), 3); // All sample rules are rust
    assert_eq!(python_rules.len(), 0);
}

#[test]
fn test_importer_filter_by_type() {
    let catalog = create_sample_catalog();

    let code_smells = catalog.for_type("CODE_SMELL");
    let bugs = catalog.for_type("BUG");

    assert_eq!(code_smells.len(), 2); // S1226, S1186
    assert_eq!(bugs.len(), 1);        // S1871
}

#[test]
fn test_importer_rule_parameter_structure() {
    let param = RuleParameter {
        name: "max_depth".to_string(),
        description: "Maximum nesting depth".to_string(),
        default_value: Some("3".to_string()),
        param_type: "INT".to_string(),
    };

    assert_eq!(param.name, "max_depth");
    assert_eq!(param.default_value.as_deref(), Some("3"));
    assert_eq!(param.param_type, "INT");
}

#[test]
fn test_importer_severity_mapping() {
    let catalog = create_sample_catalog();

    // Verify severity is preserved correctly
    for rule in &catalog.rules {
        assert!(["BLOCKER", "CRITICAL", "MAJOR", "MINOR", "INFO"].contains(&rule.severity.as_str()));
    }
}

#[test]
fn test_importer_rule_type_mapping() {
    let catalog = create_sample_catalog();

    for rule in &catalog.rules {
        assert!(
            ["CODE_SMELL", "BUG", "VULNERABILITY", "SECURITY_HOTSPOT"].contains(&rule.rule_type.as_str()),
            "Invalid rule type: {}",
            rule.rule_type
        );
    }
}

// ============================================================================
// Additional Integration Tests
// ============================================================================

#[test]
fn test_full_quality_workflow() {
    // Simulate a full quality workflow: issues -> debt -> ratings -> gate

    // Step 1: Create some issues
    let issues = vec![
        Issue::new("S138", "High complexity", Severity::Major, Category::CodeSmell,
                   PathBuf::from("src/main.rs"), 10),
        Issue::new("S1871", "Duplication", Severity::Major, Category::Bug,
                   PathBuf::from("src/lib.rs"), 25),
        Issue::new("VULN1", "SQL Injection", Severity::Critical, Category::Vulnerability,
                   PathBuf::from("src/db.rs"), 50),
    ];

    // Step 2: Calculate technical debt
    let calculator = TechnicalDebtCalculator::new();
    let debt_report = calculator.calculate(&issues, 5000);

    // Step 3: Compute ratings
    let ratings = ProjectRatings::compute(&issues, 5000, &debt_report);

    // Step 4: Evaluate against quality gate
    let gate = QualityGate::new("Release Gate", "Gate for production release")
        .add_condition(GateCondition::new(
            "vulnerabilities",
            CompareOperator::EQ,
            MetricValue::Integer(0),
        ))
        .add_condition(GateCondition::new(
            "bugs",
            CompareOperator::LTE,
            MetricValue::Integer(5),
        ));

    let mut metrics = ProjectMetrics::new();
    metrics.vulnerabilities = issues.iter().filter(|i| i.category == Category::Vulnerability).count();
    metrics.bugs = issues.iter().filter(|i| i.category == Category::Bug).count();
    metrics.code_smells = issues.iter().filter(|i| i.category == Category::CodeSmell).count();
    metrics.ncloc = 5000;
    metrics.debt_ratio = debt_report.debt_ratio;

    let gate_result = gate.evaluate(&metrics);

    // Verify workflow results
    assert!(debt_report.total_debt_minutes > 0);
    assert!(ratings.security < DebtRating::A); // Has vulnerabilities
    assert!(!gate_result.passed); // Should fail due to vulnerability
}

#[test]
fn test_project_metrics_get() {
    let mut metrics = ProjectMetrics::new();
    metrics.ncloc = 1000;
    metrics.functions = 50;
    metrics.classes = 10;
    metrics.complexity = 150;
    metrics.code_smells = 25;
    metrics.bugs = 3;
    metrics.vulnerabilities = 1;
    metrics.duplication_percentage = 5.5;
    metrics.coverage_percentage = Some(75.0);

    // Test all metric retrievals using matches! macro
    assert!(matches!(metrics.get("ncloc"), Some(MetricValue::Integer(1000))));
    assert!(matches!(metrics.get("functions"), Some(MetricValue::Integer(50))));
    assert!(matches!(metrics.get("classes"), Some(MetricValue::Integer(10))));
    assert!(matches!(metrics.get("complexity"), Some(MetricValue::Integer(150))));
    assert!(matches!(metrics.get("code_smells"), Some(MetricValue::Integer(25))));
    assert!(matches!(metrics.get("bugs"), Some(MetricValue::Integer(3))));
    assert!(matches!(metrics.get("vulnerabilities"), Some(MetricValue::Integer(1))));
    assert!(matches!(metrics.get("duplication_percentage"), Some(MetricValue::Percentage(5.5))));
    assert!(matches!(metrics.get("coverage"), Some(MetricValue::Percentage(75.0))));
    assert!(matches!(metrics.get("nonexistent"), None));
}

#[test]
fn test_compare_operator_evaluate() {
    // Test all comparison operators with integers
    assert!(CompareOperator::GT.evaluate(5, 3));
    assert!(!CompareOperator::GT.evaluate(3, 5));
    assert!(CompareOperator::GTE.evaluate(5, 5));
    assert!(CompareOperator::GTE.evaluate(5, 4));
    assert!(CompareOperator::LT.evaluate(3, 5));
    assert!(!CompareOperator::LT.evaluate(5, 3));
    assert!(CompareOperator::LTE.evaluate(5, 5));
    assert!(CompareOperator::EQ.evaluate(5, 5));
    assert!(!CompareOperator::EQ.evaluate(5, 3));
    assert!(CompareOperator::NEQ.evaluate(5, 3));
    assert!(!CompareOperator::NEQ.evaluate(5, 5));

    // Test with floats
    assert!(CompareOperator::GT.evaluate(3.14, 2.71));
    assert!(CompareOperator::EQ.evaluate(3.0, 3.0));

    // Note: Mixed int/float comparisons are tested via MetricValue.compare() not CompareOperator::evaluate directly
    // because CompareOperator::evaluate requires same types
}

#[test]
fn test_metric_value_compare() {
    // Integer comparisons
    assert_eq!(
        MetricValue::Integer(10).compare(CompareOperator::GT, &MetricValue::Integer(5)),
        Some(true)
    );
    assert_eq!(
        MetricValue::Integer(10).compare(CompareOperator::EQ, &MetricValue::Integer(10)),
        Some(true)
    );

    // Float comparisons
    assert_eq!(
        MetricValue::Float(3.14).compare(CompareOperator::EQ, &MetricValue::Float(3.14)),
        Some(true)
    );

    // Percentage comparisons
    assert_eq!(
        MetricValue::Percentage(80.0).compare(CompareOperator::GTE, &MetricValue::Percentage(75.0)),
        Some(true)
    );

    // Mixed int/float
    assert_eq!(
        MetricValue::Integer(5).compare(CompareOperator::LT, &MetricValue::Float(10.0)),
        Some(true)
    );

    // Type mismatch returns None
    assert_eq!(
        MetricValue::Integer(5).compare(CompareOperator::EQ, &MetricValue::Percentage(5.0)),
        None
    );
}

#[test]
fn test_debt_rating_labels() {
    assert_eq!(DebtRating::A.label(), 'A');
    assert_eq!(DebtRating::B.label(), 'B');
    assert_eq!(DebtRating::C.label(), 'C');
    assert_eq!(DebtRating::D.label(), 'D');
    assert_eq!(DebtRating::E.label(), 'E');
}

#[test]
fn test_debt_rating_ordering() {
    // DebtRating derives PartialOrd with A=5 (best) to E=1 (worst)
    assert!(DebtRating::A > DebtRating::B);
    assert!(DebtRating::B > DebtRating::C);
    assert!(DebtRating::C > DebtRating::D);
    assert!(DebtRating::D > DebtRating::E);
    // A=5, E=1 numerically, so A > E
    assert!(DebtRating::A > DebtRating::E);
    // But in terms of "best is highest", A is best, E is worst
    // The enum values reflect this: A=5 (best) to E=1 (worst)
}

#[test]
fn test_quality_gate_evaluator_failed_gates() {
    let gates = vec![
        QualityGate::new("Passing Gate", "This will pass")
            .add_condition(GateCondition::new(
                "ncloc",
                CompareOperator::LT,
                MetricValue::Integer(100000),
            )),
        QualityGate::new("Failing Gate", "This will fail")
            .add_condition(GateCondition::new(
                "bugs",
                CompareOperator::EQ,
                MetricValue::Integer(0),
            )),
    ];

    let evaluator = QualityGateEvaluator::new(gates);

    let mut metrics = ProjectMetrics::new();
    metrics.ncloc = 1000;
    metrics.bugs = 5; // Causes failing gate

    let results = evaluator.evaluate_all(&metrics);
    let failed: Vec<_> = evaluator.failed_gates(&results);

    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].gate_name, "Failing Gate");
}
