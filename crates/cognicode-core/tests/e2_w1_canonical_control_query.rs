//! Integration test for E2.W1: `wire_canonical_control_query()` produces
//! a `ControlQueryService` with the canonical CogniCode constraints
//! admitted, and that service evaluates source against those rules.
//!
//! This is the **CP1 first consumer** test: it pins the production
//! wiring helper so a future refactor that returns an empty registry
//! (or fails to admit the canonical rules) breaks the build.

use std::fs;
use std::path::{Path, PathBuf};

use cognicode_core::application::architecture::control_query::{
    self, wire_canonical_control_query,
};
use cognicode_core::application::architecture::{ArchitectureSource, SourceFile};

const CORE_SRC_ROOT: &str = "src";

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

fn collect_rs_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" {
                continue;
            }
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if path.file_name().and_then(|n| n.to_str()) == Some("mod.rs") {
                continue;
            }
            out.push(path);
        }
    }
}

fn build_cognicode_core_source(crate_root: &Path) -> Option<ArchitectureSource> {
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
        let rel = rel.strip_prefix(CORE_SRC_ROOT).ok()?;
        let segments: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .map(|s| s.trim_end_matches(".rs").to_string())
            .filter(|seg| seg != "mod" && seg != "lib" && seg != "main")
            .collect();
        let module_path = (!segments.is_empty()).then(|| segments.join("::"));
        files.push(SourceFile {
            file_path: rel.to_string_lossy().to_string(),
            module_path,
            source,
        });
    }
    Some(ArchitectureSource { files })
}

#[test]
fn wire_canonical_control_query_admits_three_constraints() {
    // The wiring helper must succeed and produce a registry with the
    // three canonical constraints admitted.
    let cq = wire_canonical_control_query();
    let admitted = cq.registry().admission.admitted();
    assert_eq!(
        admitted.len(),
        3,
        "wire_canonical_control_query must admit exactly 3 canonical constraints; got {:?}",
        admitted.iter().map(|c| c.id.as_str()).collect::<Vec<_>>()
    );
    let ids: Vec<&str> = admitted.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "architecture.domain_no_infrastructure",
            "architecture.domain_no_application",
            "architecture.evidence_kernel_no_presentation",
        ],
        "constraint order must match canonical_constraints() output"
    );
    // Every admitted constraint must be from a promoted admitter
    // (the canonical_promoted_admitter). This is the load-bearing
    // property: non-promoted admission is rejected, so a passing
    // registry means the canonical admitter was indeed promoted.
    for c in admitted {
        assert!(
            c.admitted_by.may_admit(),
            "constraint {} must have been admitted by a promoted admitter",
            c.id.as_str()
        );
        assert_eq!(c.admitted_by.id, "human:cognicode-architecture-wg");
    }
}

#[test]
fn wire_canonical_control_query_evaluates_self_host_to_zero_drift() {
    // The wiring helper, when run against the cognicode-core source,
    // must report Evaluated + 0 violations. This is the production
    // equivalent of `architecture_self_host_e2e::self_host_evaluator_finds_zero_drift_on_clean_source`.
    let cq = wire_canonical_control_query();

    let crate_root = match locate_crate_root() {
        Some(p) => p,
        None => {
            eprintln!("E2.W1 self-host skipped: CARGO_MANIFEST_DIR not set");
            return;
        }
    };
    let source = match build_cognicode_core_source(&crate_root) {
        Some(s) => s,
        None => {
            eprintln!("E2.W1 self-host skipped: source tree not found");
            return;
        }
    };
    assert!(
        !source.files.is_empty(),
        "no source files collected from cognicode-core"
    );

    let model = cq.query_architecture("cognicode-core", None, &source);
    match model.status {
        control_query::EvaluationStatus::Evaluated => {}
        control_query::EvaluationStatus::Incomplete => panic!(
            "E2.W1: registry returned Incomplete for self-host; constraints admitted but evaluation did not finish cleanly. unevaluated={:?} reason: status returns to Incomplete when any constraint errors during evaluation",
            model.unevaluated_constraints
        ),
    }
    assert_eq!(
        model.constraints.len(),
        3,
        "read model must include the 3 canonical constraints"
    );
    assert!(
        model.violations.is_empty(),
        "self-host must produce zero violations; got {}: {:#?}",
        model.violations.len(),
        model.violations
    );
    assert!(
        model.statements_examined > 0,
        "evaluator must examine at least one statement"
    );
}

#[test]
fn wire_canonical_control_query_evaluates_synthetic_drift() {
    // A synthetic source with `domain::service` importing `infrastructure::db`
    // must produce one violation against the
    // `architecture.domain_no_infrastructure` constraint. This pins the
    // wired registry as functionally equivalent to the per-test
    // fixture used in `architecture_self_host_e2e`.
    let cq = wire_canonical_control_query();
    let source = control_query::source_from_files(vec![(
        "src/domain/service.rs".into(),
        Some("domain::service".into()),
        "use crate::infrastructure::db::Db;\npub fn connect() -> Db { unimplemented!() }\n".into(),
    )]);
    let model = cq.query_architecture("synth", None, &source);
    assert_eq!(model.constraints.len(), 3);
    assert_eq!(
        model.violations.len(),
        1,
        "expected exactly one violation for the synthetic drift; got {:#?}",
        model.violations
    );
    assert_eq!(
        model.violations[0].constraint_id, "architecture.domain_no_infrastructure",
        "the violation must be attributed to the layer-dependency constraint"
    );
}
