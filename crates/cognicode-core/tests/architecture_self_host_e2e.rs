//! Self-hosting architectural evaluation (WU5).
//!
//! This test exercises the e77 architecture evaluator against the
//! **actual source** of `cognicode-core` — the same crate the
//! evaluator lives in. It is the first real consumer of the
//! self-hosting seam (e76) and the only meaningful signal that the
//! evaluator is load-bearing: a green test means the evaluator is
//! *checking real source*, not running on trivial fixtures.
//!
//! The three constraints admitted here are the **canonical CogniCode
//! architecture rules** declared in
//! `docs/analysis/e77-architecture-ownership-map.md`. They are the
//! rules a maintainer would manually enforce today; e77 makes them
//! mechanical.
//!
//! If a rule ever fires on the current source, **the test fails** and
//! a real drift must be fixed. Conversely, if the evaluator cannot
//! find any drift when the source has one, the test fails — that is
//! the signal that the evaluator is *not* load-bearing and must be
//! downgraded.
//!
//! The test is also a property: re-running it on the same source must
//! always yield the same result. Synthetic evidence ids are derived
//! from a stable hash (see `evaluator::stable_hash_id`) so report
//! equality is testable.

use std::fs;
use std::path::{Path, PathBuf};

use cognicode_core::application::architecture::{
    ArchitectureRegistry, ArchitectureSource, SourceFile, canonical_constraints,
    canonical_promoted_admitter, cr06_allowlist,
};
use cognicode_core::domain::architecture::{Admitter, AdmitterRole, LayerId};
use cognicode_core::domain::findings::{EvidenceClass, FindingGate, RiskLevel};

const CORE_SRC_ROOT: &str = "src";

/// Walk a directory recursively and collect every `.rs` file (skipping
/// `target/`, hidden directories, and `mod.rs` files — `mod.rs` would
/// double-count modules).
fn collect_rs_files(root: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden dirs and well-known build dirs.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" {
                continue;
            }
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            // Skip `mod.rs` files: the test is interested in modules,
            // not the directory entries that re-export them. Each
            // module shows up as a `.rs` file at its own level.
            if path.file_name().and_then(|n| n.to_str()) == Some("mod.rs") {
                continue;
            }
            out.push(path);
        }
    }
}

/// Derive a `module_path` from a path relative to `CORE_SRC_ROOT`.
/// E.g. `src/domain/findings/finding.rs` → `domain::findings::finding`.
fn module_path_for(rel: &Path) -> Option<String> {
    let rel = rel.strip_prefix(CORE_SRC_ROOT).ok()?;
    let mut parts: Vec<String> = Vec::new();
    for comp in rel.components() {
        let s = comp.as_os_str().to_string_lossy().to_string();
        // Drop the `.rs` extension on the last component.
        let s = s.trim_end_matches(".rs").to_string();
        parts.push(s);
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("::"))
    }
}

/// Build an `ArchitectureSource` from the live `cognicode-core`
/// source tree. Returns `None` if the tree cannot be located (e.g.
/// the test is run from outside the crate root); the test is skipped
/// in that case rather than failed, because the self-host property
/// requires the source on disk.
fn build_self_host_source(crate_root: &Path) -> Option<ArchitectureSource> {
    let src_root = crate_root.join(CORE_SRC_ROOT);
    if !src_root.exists() {
        return None;
    }
    let mut files = Vec::new();
    let mut paths = Vec::new();
    collect_rs_files(&src_root, &mut paths);
    for path in paths {
        let rel = path.strip_prefix(crate_root).ok()?;
        let source = fs::read_to_string(&path).ok()?;
        let module_path = module_path_for(rel);
        // CR-06: emit file_path relative to `src/` so the value
        // matches what the CR-06 allowlist (and any other
        // architecture allowlist, in current or future constraints)
        // records as `file_path`. The previous shape
        // (`src/application/foo.rs`) was internal-relative; the
        // module path (`application::foo`) is the canonical surface.
        let file_path_rel = rel
            .strip_prefix("src")
            .unwrap_or(rel)
            .to_string_lossy()
            .to_string();
        files.push(SourceFile {
            file_path: file_path_rel,
            module_path,
            source,
        });
    }
    Some(ArchitectureSource { files })
}

/// Locate the crate root by walking up from `CARGO_MANIFEST_DIR`
/// until we find `Cargo.toml`. This makes the test work both when
/// run from the crate root and from a parent workspace.
fn locate_crate_root() -> Option<PathBuf> {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let mut here: Option<&Path> = Some(Path::new(&manifest));
    while let Some(p) = here {
        if p.join("Cargo.toml").exists() {
            return Some(p.to_path_buf());
        }
        here = p.parent();
    }
    None
}

/// The self-host gate.
///
/// e82.1 closed the last real drift (P1.4: `domain::behaviors::runtime` →
/// `application::intelligence_log::CausalRecorder`), so this test is no longer
/// `#[ignore]`d: it now runs as part of normal verification and asserts that the
/// evaluator finds **zero** drift in `cognicode-core`'s own source.
///
/// A failing run means either a new drift, or that the evaluator stopped
/// checking real source; both must be surfaced, never absorbed.
#[test]
fn self_host_evaluator_finds_zero_drift_on_clean_source() {
    let crate_root = match locate_crate_root() {
        Some(p) => p,
        None => {
            eprintln!("self-host skipped: CARGO_MANIFEST_DIR not set");
            return;
        }
    };
    let source = match build_self_host_source(&crate_root) {
        Some(s) => s,
        None => {
            eprintln!(
                "self-host skipped: source tree not found at {}",
                crate_root.display()
            );
            return;
        }
    };
    // The test is only meaningful if we actually have source files.
    assert!(!source.files.is_empty(), "no source files collected");

    let mut registry = ArchitectureRegistry::new().with_temporary_exceptions(cr06_allowlist::exceptions());
    let admitter = canonical_promoted_admitter();
    let clock = cognicode_core::application::architecture::admission::SystemArchitectureClock;
    for candidate in canonical_constraints() {
        let out = registry.admission.admit(candidate, &admitter, &clock);
        assert!(
            out.result.is_ok(),
            "self-host: failed to admit canonical constraint: {:?}",
            out.result
        );
    }

    let gate = FindingGate::new(EvidenceClass::B, RiskLevel::Medium);
    let mut all_violations: Vec<String> = Vec::new();
    for constraint in registry.admission.admitted() {
        let report = registry
            .evaluate(constraint, &source)
            .expect("self-host evaluation must succeed");
        for violation in &report.violations {
            // Record every violation. The post-e77.1 evaluator
            // emits violations as the primary output; the gate is
            // only consulted by downstream assembly. We log all
            // violations so the self-host test still flags the
            // real drifts in `cognicode-core`'s source.
            //
            // CR-06: the registry filters via `cr06_allowlist`. A
            // drift that appears here is *new* and must be added to
            // the allowlist (with owner+rationale+expiry) or fixed
            // in code. The two states are not equivalent: the
            // allowlist is an inventory, not a permission slip.
            all_violations.push(format!(
                "{} @ {}:{} -> {} (rationale: {})",
                violation.finding_kind.as_str(),
                violation.file_path,
                violation.line,
                violation.dependency_path,
                violation.rationale,
            ));
        }
    }
    let _ = gate; // kept for parity with prior shape; unused on violations.

    assert!(
        all_violations.is_empty(),
        "self-host: architecture evaluator found drifts in cognicode-core's own source:\n  - {}",
        all_violations.join("\n  - ")
    );
}

#[test]
fn self_host_evaluator_finds_real_drift_in_synthetic_fixture() {
    // The counterpart to the ignored test above: a *positive*
    // demonstration that the evaluator is load-bearing. If the
    // evaluator could not find any drift in a fixture that is
    // obviously wrong, it would not be load-bearing and e77 would
    // have to be downgraded. This test is the gate that proves the
    // evaluator is doing real work, not running on trivial fixtures.
    use cognicode_core::domain::architecture::{
        Admitter, AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind,
        LayerDependencyRule, LayerId,
    };

    let candidate = cognicode_core::domain::architecture::ConstraintCandidate {
        id: ArchitectureConstraintId::new("architecture.test_layer").unwrap(),
        kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
            from_layer: LayerId::Domain,
            forbidden_targets: vec![LayerId::Infrastructure],
            rationale: "test".into(),
        }),
        adr_ref: Some("ADR-046".into()),
        proposed_by: "human:test".into(),
    };

    let mut registry = ArchitectureRegistry::new();
    let admitter = Admitter {
        id: "human:test".into(),
        role: AdmitterRole::HumanPromoter,
    };
    let clock = cognicode_core::application::architecture::admission::SystemArchitectureClock;
    let out = registry.admission.admit(candidate, &admitter, &clock);
    assert!(out.result.is_ok());
    let constraint = registry.admission.admitted().first().unwrap().clone();

    // Synthetic fixture: a domain file that imports infrastructure.
    let source = ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/domain/example.rs".into(),
            module_path: Some("domain::example".into()),
            source: "use crate::infrastructure::db::Pool;\n".into(),
        }],
    };

    let report = registry
        .evaluate(&constraint, &source)
        .expect("evaluator must succeed on valid input");
    assert_eq!(
        report.violations.len(),
        1,
        "evaluator must find the synthetic drift"
    );
    assert_eq!(
        report.violations[0].finding_kind.as_str(),
        "architecture.layer_dependency"
    );
}

#[test]
fn self_host_admitted_constraints_are_promoted() {
    // The admission flow is part of the self-host contract: a
    // non-promoted admitter cannot admit a constraint, so even if
    // someone tries to sneak in a constraint via the wrong path,
    // the evaluator never sees it.
    let mut registry = ArchitectureRegistry::new();
    let candidate = canonical_constraints().into_iter().next().unwrap();
    let non_promoted = Admitter {
        id: "ai:test".into(),
        role: AdmitterRole::Other,
    };
    let out = registry.admission.admit(
        candidate.clone(),
        &non_promoted,
        &cognicode_core::application::architecture::admission::SystemArchitectureClock,
    );
    assert!(out.result.is_err());
    assert!(!registry.is_admitted(&candidate));
}

/// CR-06 T7 (synthetic drift detector for application_no_infrastructure).
///
/// Pins the load-bearing property of the rule added in CR-06: when an
/// `application/` module imports `crate::infrastructure::...`, the
/// evaluator MUST emit a violation with `constraint_id =
/// architecture.application_no_infrastructure`. If this test ever
/// fails (no violation found), the rule is silently no-op — the gate
/// has stopped working and must be repaired, not relaxed.
#[test]
fn cr06_synthetic_drift_application_to_infrastructure_is_detected() {
    let mut registry = ArchitectureRegistry::new();
    let admitter = canonical_promoted_admitter();
    let clock = cognicode_core::application::architecture::admission::SystemArchitectureClock;
    let candidate = canonical_constraints()
        .into_iter()
        .find(|c| c.id.as_str() == "architecture.application_no_infrastructure")
        .expect("CR-06: application_no_infrastructure must be canonical");
    let out = registry.admission.admit(candidate, &admitter, &clock);
    assert!(out.result.is_ok());
    let constraint = registry.admission.admitted().first().unwrap().clone();

    // Synthetic fixture: an application module importing
    // `crate::infrastructure::parser::Language`. The shape of this
    // string must not change casually — it is the contract of the
    // gate.
    let source = ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/application/example.rs".into(),
            module_path: Some("application::example".into()),
            source: "use crate::infrastructure::parser::Language;\n".into(),
        }],
    };

    let report = registry
        .evaluate(&constraint, &source)
        .expect("evaluator must succeed on valid input");
    assert_eq!(
        report.violations.len(),
        1,
        "application_no_infrastructure must detect application→infrastructure drift; got 0"
    );
    let v = &report.violations[0];
    assert_eq!(
        v.constraint_id.as_str(),
        "architecture.application_no_infrastructure"
    );
    assert_eq!(v.from_layer, LayerId::Application);
}

/// CR-06 T7 (synthetic drift detector for application_no_interface).
///
/// Pins the load-bearing property of the rule: when an
/// `application/` module imports `crate::bin::...` (the layer used to
/// represent `interface::mcp` per `LayerId::from_module_path`), the
/// evaluator MUST emit a violation with `constraint_id =
/// architecture.application_no_interface`. This test proves the gate
/// is live and not a no-op.
#[test]
fn cr06_synthetic_drift_application_to_interface_is_detected() {
    let mut registry = ArchitectureRegistry::new();
    let admitter = canonical_promoted_admitter();
    let clock = cognicode_core::application::architecture::admission::SystemArchitectureClock;
    let candidate = canonical_constraints()
        .into_iter()
        .find(|c| c.id.as_str() == "architecture.application_no_interface")
        .expect("CR-06: application_no_interface must be canonical");
    let out = registry.admission.admit(candidate, &admitter, &clock);
    assert!(out.result.is_ok());
    let constraint = registry.admission.admitted().first().unwrap().clone();

    // Synthetic fixture: an application module importing
    // `crate::bin::something_interface`. The gate must fire.
    let source = ArchitectureSource {
        files: vec![SourceFile {
            file_path: "src/application/example.rs".into(),
            module_path: Some("application::example".into()),
            source: "use crate::bin::something_interface;\n".into(),
        }],
    };

    let report = registry
        .evaluate(&constraint, &source)
        .expect("evaluator must succeed on valid input");
    assert_eq!(
        report.violations.len(),
        1,
        "application_no_interface must detect application→interface drift; got 0"
    );
    let v = &report.violations[0];
    assert_eq!(
        v.constraint_id.as_str(),
        "architecture.application_no_interface"
    );
    assert_eq!(v.from_layer, LayerId::Application);
}
