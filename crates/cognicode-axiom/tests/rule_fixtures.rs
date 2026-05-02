use cognicode_rule_test_harness::fixture::Fixture;
use cognicode_rule_test_harness::runner::RuleRunner;
use std::path::Path;

#[test]
fn test_rust_rule_fixtures() {
    let runner = RuleRunner::new();
    // Resolve from workspace root (2 levels up from crates/cognicode-axiom/)
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()   // crates/
        .parent().unwrap();  // workspace root
    let fixtures_dir = workspace_root.join("sandbox/fixtures/rules/rust");

    if !fixtures_dir.exists() {
        eprintln!("Fixtures directory not found at {:?}, skipping", fixtures_dir);
        return;
    }
    eprintln!("Fixtures directory: {:?}", fixtures_dir);

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut reports: Vec<String> = Vec::new();

    let fixture_dirs = match std::fs::read_dir(fixtures_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()),
        Err(e) => {
            eprintln!("Error reading fixtures directory: {}", e);
            return;
        }
    };

    for entry in fixture_dirs {
        let entry_path = entry.path();

        // Skip if not a fixture (no expected.json)
        if !entry_path.join("expected.json").exists() {
            continue;
        }

        let fixture = match Fixture::load(&entry_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error loading fixture from {}: {}", entry_path.display(), e);
                continue;
            }
        };

        let mut report = cognicode_rule_test_harness::report::TestReport::new(&fixture.rule_id);

        for case in &fixture.test_cases {
            let issues = runner
                .run_rule_on_file(&fixture.rule_id, &entry_path, &case.file)
                .unwrap_or_else(|e| {
                    eprintln!("Error running {}: {}", case.name, e);
                    Vec::new()
                });
            report.add_result(case, &issues);
        }

        reports.push(report.summary());

        // Print detailed results for failures
        if !report.all_passed() {
            for result in &report.results {
                if !result.passed {
                    eprintln!(
                        "FAILED: {} in {} - {}",
                        result.name, fixture.rule_id, entry_path.display()
                    );
                    for error in &result.errors {
                        eprintln!("  Error: {}", error);
                    }
                }
            }
        }

        total_passed += report.passed;
        total_failed += report.failed;
    }

    // Print all summaries
    for summary in &reports {
        println!("{}", summary);
    }

    println!("\nTotal: {} passed, {} failed", total_passed, total_failed);
    assert_eq!(
        total_failed, 0,
        "Some tests failed! See output above for details."
    );
}

#[test]
fn test_rule_runner_discovery() {
    let runner = RuleRunner::new();
    let rule_ids = runner.get_rule_ids();

    // Should have discovered some rules
    assert!(
        !rule_ids.is_empty(),
        "Expected some rules to be discovered"
    );

    println!("Discovered {} rules", rule_ids.len());
}

#[test]
fn test_js_rule_fixtures() {
    let runner = RuleRunner::new();
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap();
    let fixtures_dir = workspace_root.join("sandbox/fixtures/rules/javascript");

    if !fixtures_dir.exists() {
        eprintln!("Fixtures directory not found at {:?}, skipping", fixtures_dir);
        return;
    }
    eprintln!("Fixtures directory: {:?}", fixtures_dir);

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut reports: Vec<String> = Vec::new();

    let fixture_dirs = match std::fs::read_dir(fixtures_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()),
        Err(e) => {
            eprintln!("Error reading fixtures directory: {}", e);
            return;
        }
    };

    for entry in fixture_dirs {
        let entry_path = entry.path();

        if !entry_path.join("expected.json").exists() {
            continue;
        }

        let fixture = match Fixture::load(&entry_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error loading fixture from {}: {}", entry_path.display(), e);
                continue;
            }
        };

        let mut report = cognicode_rule_test_harness::report::TestReport::new(&fixture.rule_id);

        for case in &fixture.test_cases {
            let issues = runner
                .run_rule_on_file(&fixture.rule_id, &entry_path, &case.file)
                .unwrap_or_else(|e| {
                    eprintln!("Error running {}: {}", case.name, e);
                    Vec::new()
                });
            report.add_result(case, &issues);
        }

        reports.push(report.summary());

        if !report.all_passed() {
            for result in &report.results {
                if !result.passed {
                    eprintln!(
                        "FAILED: {} in {} - {}",
                        result.name, fixture.rule_id, entry_path.display()
                    );
                    for error in &result.errors {
                        eprintln!("  Error: {}", error);
                    }
                }
            }
        }

        total_passed += report.passed;
        total_failed += report.failed;
    }

    for summary in &reports {
        println!("{}", summary);
    }

    println!("\nTotal JS: {} passed, {} failed", total_passed, total_failed);
    assert_eq!(
        total_failed, 0,
        "Some JS tests failed! See output above for details."
    );
}

#[test]
fn test_java_rule_fixtures() {
    let runner = RuleRunner::new();
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap();
    let fixtures_dir = workspace_root.join("sandbox/fixtures/rules/java");

    if !fixtures_dir.exists() {
        eprintln!("Fixtures directory not found at {:?}, skipping", fixtures_dir);
        return;
    }
    eprintln!("Fixtures directory: {:?}", fixtures_dir);

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut reports: Vec<String> = Vec::new();

    let fixture_dirs = match std::fs::read_dir(fixtures_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()),
        Err(e) => {
            eprintln!("Error reading fixtures directory: {}", e);
            return;
        }
    };

    for entry in fixture_dirs {
        let entry_path = entry.path();

        if !entry_path.join("expected.json").exists() {
            continue;
        }

        let fixture = match Fixture::load(&entry_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error loading fixture from {}: {}", entry_path.display(), e);
                continue;
            }
        };

        let mut report = cognicode_rule_test_harness::report::TestReport::new(&fixture.rule_id);

        for case in &fixture.test_cases {
            let issues = runner
                .run_rule_on_file(&fixture.rule_id, &entry_path, &case.file)
                .unwrap_or_else(|e| {
                    eprintln!("Error running {}: {}", case.name, e);
                    Vec::new()
                });
            report.add_result(case, &issues);
        }

        reports.push(report.summary());

        if !report.all_passed() {
            for result in &report.results {
                if !result.passed {
                    eprintln!(
                        "FAILED: {} in {} - {}",
                        result.name, fixture.rule_id, entry_path.display()
                    );
                    for error in &result.errors {
                        eprintln!("  Error: {}", error);
                    }
                }
            }
        }

        total_passed += report.passed;
        total_failed += report.failed;
    }

    for summary in &reports {
        println!("{}", summary);
    }

    println!("\nTotal Java: {} passed, {} failed", total_passed, total_failed);
    assert_eq!(
        total_failed, 0,
        "Some Java tests failed! See output above for details."
    );
}

#[test]
fn test_python_rule_fixtures() {
    let runner = RuleRunner::new();
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap();
    let fixtures_dir = workspace_root.join("sandbox/fixtures/rules/python");

    if !fixtures_dir.exists() {
        eprintln!("Fixtures directory not found at {:?}, skipping", fixtures_dir);
        return;
    }
    eprintln!("Fixtures directory: {:?}", fixtures_dir);

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut reports: Vec<String> = Vec::new();

    let fixture_dirs = match std::fs::read_dir(fixtures_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()),
        Err(e) => {
            eprintln!("Error reading fixtures directory: {}", e);
            return;
        }
    };

    for entry in fixture_dirs {
        let entry_path = entry.path();

        if !entry_path.join("expected.json").exists() {
            continue;
        }

        let fixture = match Fixture::load(&entry_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error loading fixture from {}: {}", entry_path.display(), e);
                continue;
            }
        };

        let mut report = cognicode_rule_test_harness::report::TestReport::new(&fixture.rule_id);

        for case in &fixture.test_cases {
            let issues = runner
                .run_rule_on_file(&fixture.rule_id, &entry_path, &case.file)
                .unwrap_or_else(|e| {
                    eprintln!("Error running {}: {}", case.name, e);
                    Vec::new()
                });
            report.add_result(case, &issues);
        }

        reports.push(report.summary());

        if !report.all_passed() {
            for result in &report.results {
                if !result.passed {
                    eprintln!(
                        "FAILED: {} in {} - {}",
                        result.name, fixture.rule_id, entry_path.display()
                    );
                    for error in &result.errors {
                        eprintln!("  Error: {}", error);
                    }
                }
            }
        }

        total_passed += report.passed;
        total_failed += report.failed;
    }

    for summary in &reports {
        println!("{}", summary);
    }

    println!("\nTotal Python: {} passed, {} failed", total_passed, total_failed);
    assert_eq!(
        total_failed, 0,
        "Some Python tests failed! See output above for details."
    );
}

#[test]
fn test_go_rule_fixtures() {
    let runner = RuleRunner::new();
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .parent().unwrap();
    let fixtures_dir = workspace_root.join("sandbox/fixtures/rules/go");

    if !fixtures_dir.exists() {
        eprintln!("Fixtures directory not found at {:?}, skipping", fixtures_dir);
        return;
    }
    eprintln!("Fixtures directory: {:?}", fixtures_dir);

    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut reports: Vec<String> = Vec::new();

    let fixture_dirs = match std::fs::read_dir(fixtures_dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()),
        Err(e) => {
            eprintln!("Error reading fixtures directory: {}", e);
            return;
        }
    };

    for entry in fixture_dirs {
        let entry_path = entry.path();

        if !entry_path.join("expected.json").exists() {
            continue;
        }

        let fixture = match Fixture::load(&entry_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error loading fixture from {}: {}", entry_path.display(), e);
                continue;
            }
        };

        let mut report = cognicode_rule_test_harness::report::TestReport::new(&fixture.rule_id);

        for case in &fixture.test_cases {
            let issues = runner
                .run_rule_on_file(&fixture.rule_id, &entry_path, &case.file)
                .unwrap_or_else(|e| {
                    eprintln!("Error running {}: {}", case.name, e);
                    Vec::new()
                });
            report.add_result(case, &issues);
        }

        reports.push(report.summary());

        if !report.all_passed() {
            for result in &report.results {
                if !result.passed {
                    eprintln!(
                        "FAILED: {} in {} - {}",
                        result.name, fixture.rule_id, entry_path.display()
                    );
                    for error in &result.errors {
                        eprintln!("  Error: {}", error);
                    }
                }
            }
        }

        total_passed += report.passed;
        total_failed += report.failed;
    }

    for summary in &reports {
        println!("{}", summary);
    }

    println!("\nTotal Go: {} passed, {} failed", total_passed, total_failed);
    assert_eq!(
        total_failed, 0,
        "Some Go tests failed! See output above for details."
    );
}
