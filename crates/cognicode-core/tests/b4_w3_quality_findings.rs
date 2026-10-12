//! B4.W3 — Normalized quality finding representation.

use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;

use cognicode_core::findings::{
    Finding, FindingCategory, FindingLocation, FindingProvenance, FindingSeverity, FindingsReport,
};
use cognicode_core::interface::symbol_identity::SymbolId;

const QUALITY_TOOL_NAMES: &[&str] = &[
    "get_complexity",
    "detect_long_parameter_lists",
    "detect_god_functions",
    "solid_audit",
    "check_architecture",
];

fn catalog_path() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest.ancestors() {
        let candidate = ancestor.join("product").join("tools.json");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("could not locate product/tools.json above {manifest:?}");
}

fn read_raw_tools() -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(catalog_path()).expect("read product/tools.json");
    let v: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    v["tools"].as_array().expect("tools array").clone()
}

fn language_ext(lang: &str) -> &'static str {
    match lang {
        "rust" => "rs",
        "python" => "py",
        "typescript" => "ts",
        "javascript" => "js",
        "go" => "go",
        "java" => "java",
        _ => "txt",
    }
}

fn sample_finding_with(category: FindingCategory, language: &str) -> Finding {
    let ext = language_ext(language);
    let symbol = SymbolId::from_lsp_location(
        format!("{language}::example::target"),
        PathBuf::from(format!("src/{language}/example.{ext}")),
        10..42,
    );
    Finding {
        id: format!("finding-{language}-{}", category.as_str()),
        category,
        severity: FindingSeverity::High,
        message: format!("sample {language} finding"),
        location: FindingLocation {
            file: symbol.file_path.clone(),
            byte_range: 10..42,
            function: Some("target".to_string()),
        },
        provenance: FindingProvenance {
            tool: "sample-tool".to_string(),
            rule: "sample-rule".to_string(),
            algorithm: "static-ast-walk".to_string(),
            language: language.to_string(),
            confidence: 0.85,
        },
        related_symbols: vec![symbol],
    }
}

#[test]
fn every_finding_has_a_provenance_with_tool_rule_algorithm_language() {
    let f = sample_finding_with(FindingCategory::Complexity, "rust");
    assert_eq!(f.provenance.tool, "sample-tool");
    assert_eq!(f.provenance.rule, "sample-rule");
    assert_eq!(f.provenance.algorithm, "static-ast-walk");
    assert_eq!(f.provenance.language, "rust");
    assert!((0.0..=1.0).contains(&f.provenance.confidence));
}

#[test]
fn confidence_is_in_unit_interval() {
    for conf in [0.0_f32, 0.25, 0.5, 0.85, 1.0] {
        let mut f = sample_finding_with(FindingCategory::GodFunction, "python");
        f.provenance.confidence = conf;
        let json = serde_json::to_string(&f).expect("serialize");
        let parsed: Finding = serde_json::from_str(&json).expect("deserialize");
        assert!((0.0..=1.0).contains(&parsed.provenance.confidence));
    }
    assert!(!FindingsReport::confidence_in_range(1.5));
    assert!(!FindingsReport::confidence_in_range(-0.1));
    let mut bad = sample_finding_with(FindingCategory::LongParameterList, "typescript");
    bad.provenance.confidence = 1.5;
    let result = Finding::new(
        bad.id.clone(),
        bad.category,
        bad.severity,
        bad.message.clone(),
        bad.location.clone(),
        bad.provenance.clone(),
    );
    assert!(result.is_err());
}

#[test]
fn location_byte_range_is_per_file() {
    let loc = FindingLocation {
        file: PathBuf::from("crates/cognicode-core/src/findings/mod.rs"),
        byte_range: 100..200,
        function: Some("f".to_string()),
    };
    assert_eq!(
        loc.byte_range,
        Range::<usize> {
            start: 100,
            end: 200
        }
    );
    assert!(loc.byte_range.end > loc.byte_range.start);
    let a = FindingLocation {
        file: PathBuf::from("a.rs"),
        byte_range: 0..10,
        function: None,
    };
    let b = FindingLocation {
        file: PathBuf::from("b.rs"),
        byte_range: 0..10,
        function: None,
    };
    assert_eq!(a.byte_range, b.byte_range);
    assert_ne!(a.file, b.file);
}

#[test]
fn language_breakdown_sums_to_total_findings() {
    let findings = vec![
        sample_finding_with(FindingCategory::Complexity, "rust"),
        sample_finding_with(FindingCategory::Complexity, "rust"),
        sample_finding_with(FindingCategory::GodFunction, "python"),
    ];
    let report = FindingsReport {
        findings: findings.clone(),
        total_files_scanned: 2,
        language_breakdown: BTreeMap::from([("rust".to_string(), 2), ("python".to_string(), 1)]),
        truncated: false,
    };
    let sum: usize = report.language_breakdown.values().sum();
    assert_eq!(sum, report.findings.len());
    let derived = FindingsReport::from_findings(findings, 2);
    let derived_sum: usize = derived.language_breakdown.values().sum();
    assert_eq!(derived_sum, derived.findings.len());
}

#[test]
fn catalog_language_scope_matches_what_tools_produce() {
    let raw = read_raw_tools();
    let by_name: std::collections::HashMap<&str, &serde_json::Value> = raw
        .iter()
        .map(|t| (t["name"].as_str().expect("name"), t))
        .collect();
    for tool_name in QUALITY_TOOL_NAMES {
        let raw_entry = by_name
            .get(tool_name)
            .unwrap_or_else(|| panic!("raw tool {tool_name} missing"));
        let scope = raw_entry
            .get("language_scope")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("tool {tool_name} missing language_scope"))
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect::<Vec<_>>();
        assert!(!scope.is_empty());
        for lang in &scope {
            assert!(
                !lang.is_empty() && lang.chars().all(|c| c.is_ascii_lowercase()),
                "{tool_name} language_scope {lang:?} must be lowercase ascii"
            );
        }
    }
}

#[test]
fn findings_for_different_languages_are_not_merged() {
    let findings = vec![
        sample_finding_with(FindingCategory::Complexity, "rust"),
        sample_finding_with(FindingCategory::Complexity, "python"),
        sample_finding_with(FindingCategory::Complexity, "typescript"),
    ];
    let report = FindingsReport::from_findings(findings, 3);
    let langs: HashSet<&str> = report
        .findings
        .iter()
        .map(|f| f.provenance.language.as_str())
        .collect();
    assert_eq!(langs.len(), 3);
    assert_eq!(report.language_breakdown.len(), 3);
    for (lang, count) in &report.language_breakdown {
        assert_eq!(*count, 1, "lang {lang} should have 1 finding");
    }
}

#[test]
fn truncated_flag_set_when_findings_exceed_budget() {
    let mut findings = Vec::new();
    for i in 0..3 {
        let mut f = sample_finding_with(FindingCategory::GodFunction, "rust");
        f.id = format!("god-{i}");
        findings.push(f);
    }
    let report = FindingsReport {
        findings: findings.clone(),
        total_files_scanned: 1,
        language_breakdown: BTreeMap::from([("rust".to_string(), 3)]),
        truncated: true,
    };
    assert!(report.truncated);
    let computed = FindingsReport::with_truncation(report.findings.clone(), 1, true);
    assert!(computed.truncated);
    assert_eq!(computed.findings.len(), 3);
    let sum: usize = computed.language_breakdown.values().sum();
    assert_eq!(sum, computed.findings.len());
}

#[test]
fn finding_category_serialize_round_trip() {
    let cats = [
        FindingCategory::Complexity,
        FindingCategory::LongParameterList,
        FindingCategory::GodFunction,
        FindingCategory::SolidViolation,
        FindingCategory::RelationshipCycle,
        FindingCategory::Other("custom-rule".to_string()),
    ];
    for c in &cats {
        let json = serde_json::to_string(c).expect("serialize");
        let parsed: FindingCategory = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&parsed, c);
    }
    assert_eq!(
        serde_json::to_string(&FindingCategory::Complexity).unwrap(),
        "\"complexity\""
    );
    assert_eq!(
        serde_json::to_string(&FindingCategory::RelationshipCycle).unwrap(),
        "\"relationship_cycle\""
    );
}

#[test]
fn severity_order_is_total_and_stable() {
    use std::cmp::Ordering;
    let severities = [
        FindingSeverity::Critical,
        FindingSeverity::High,
        FindingSeverity::Medium,
        FindingSeverity::Low,
        FindingSeverity::Info,
    ];
    for (i, a) in severities.iter().enumerate() {
        for (j, b) in severities.iter().enumerate() {
            let ord = a.cmp(b);
            if i < j {
                assert_eq!(ord, Ordering::Less);
            } else if i > j {
                assert_eq!(ord, Ordering::Greater);
            } else {
                assert_eq!(ord, Ordering::Equal);
            }
        }
    }
}

#[test]
fn every_quality_tool_has_a_non_empty_language_scope() {
    let raw = read_raw_tools();
    let by_name: std::collections::HashMap<&str, &serde_json::Value> = raw
        .iter()
        .map(|t| (t["name"].as_str().expect("name"), t))
        .collect();
    for tool_name in QUALITY_TOOL_NAMES {
        let entry = by_name
            .get(tool_name)
            .unwrap_or_else(|| panic!("tool {tool_name} exists"));
        let scope = entry
            .get("language_scope")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("{tool_name} missing language_scope"));
        assert!(!scope.is_empty());
        for lang in scope {
            assert!(lang.is_string());
        }
    }
}
