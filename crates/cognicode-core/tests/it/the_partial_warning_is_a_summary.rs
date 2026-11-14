//! The warning about an incomplete build is a summary, not a dump.
//!
//! ## The defect
//!
//! `warn_incomplete` wrote every hole of the build in one line of
//! stderr: the files the walk could not read, the references it could
//! not tie to a declaration, and the relations the repository refused.
//!
//! Measured over this repository, `cognicode graph hierarchy
//! rename_symbol` wrote 5 543 281 bytes to stderr in ten lines, and the
//! line of the references was 5 526 492 characters long — 46 328 holes
//! in a row. The error the person had to read was on the line after it.
//!
//! ## What is measured
//!
//! - The warning does not grow past its cap.
//! - The count it publishes is exact, even when the sample is cut.
//! - It shows a sample of the holes it counted.

use cognicode_core::application::services::analyze::CoverageFacts;
use cognicode_core::domain::aggregates::call_graph::SymbolId;
use cognicode_core::domain::value_objects::dependency_type::DependencyType;
use cognicode_core::domain::value_objects::pending_reference::{PendingClass, PendingReference};
use cognicode_core::domain::value_objects::rejected_relation::RejectedRelation;
use cognicode_core::infrastructure::graph::per_file_graph::{SkipReason, SkippedFile};
use cognicode_core::interface::project_graph::{MAX_WARNING_BYTES, incomplete_warnings};

/// How many holes a build of this repository leaves behind.
///
/// It is the number the dogfooding measured on the tree, not a round
/// number: a warning that has to survive a real build is not one that
/// survives a fixture.
const HOLES: usize = 46_328;

/// A path long enough to be worth the bytes it takes.
const PATH: &str =
    "/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode/sandbox/fixtures/ansible-playbook/site.yml";

/// The facts of a build that could not tie `HOLES` references to a
/// declaration.
fn facts_with_pending_references() -> CoverageFacts {
    CoverageFacts {
        skipped: Vec::new(),
        pending: (0..HOLES)
            .map(|index| PendingReference {
                origin_file: PATH.to_string(),
                origin_line: Some(index as u32),
                source: SymbolId::new(format!("call_{index}")),
                target: format!("ansible:role:playbook_{index}"),
                kind: DependencyType::References,
                class: PendingClass::Unresolved,
            })
            .collect(),
        rejected: Vec::new(),
    }
}

/// The facts of a build that could not read `HOLES` files.
fn facts_with_skipped_files() -> CoverageFacts {
    CoverageFacts {
        skipped: (0..HOLES)
            .map(|index| SkippedFile {
                path: format!("{PATH}#{index}"),
                reason: SkipReason::Read("permission denied".to_string()),
            })
            .collect(),
        pending: Vec::new(),
        rejected: Vec::new(),
    }
}

/// The facts of a build whose repository refused `HOLES` relations.
fn facts_with_rejected_relations() -> CoverageFacts {
    CoverageFacts {
        skipped: Vec::new(),
        pending: Vec::new(),
        rejected: (0..HOLES)
            .map(|index| RejectedRelation {
                source: SymbolId::new(format!("source_{index}")),
                target: SymbolId::new(format!("target_{index}")),
                kind: DependencyType::Calls,
                reason: "the name is declared in two places".to_string(),
            })
            .collect(),
    }
}

/// The beginning of a line, for a failure message that does not print
/// the whole line twice.
fn head(line: &str) -> &str {
    let mut end = line.len().min(240);
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    &line[..end]
}

/// The warning about the references is a summary of them.
#[test]
fn the_warning_about_the_references_is_a_summary_of_them() {
    let warnings = incomplete_warnings(&facts_with_pending_references());
    assert_eq!(warnings.len(), 1, "one line per kind of hole: {warnings:?}");
    let line = &warnings[0];
    assert!(
        line.len() <= MAX_WARNING_BYTES,
        "the warning is {} bytes long, over the cap of {MAX_WARNING_BYTES}: {}",
        line.len(),
        head(line)
    );
    assert!(
        line.contains(&format!("{HOLES} reference(s) without a declaration")),
        "the warning publishes the exact count: {}",
        head(line)
    );
    assert!(
        line.contains("site.yml:0 ansible:role:playbook_0"),
        "the warning shows a sample of the holes it counted: {}",
        head(line)
    );
}

/// The warning about the files the walk could not read is a summary of
/// them.
#[test]
fn the_warning_about_the_skipped_files_is_a_summary_of_them() {
    let warnings = incomplete_warnings(&facts_with_skipped_files());
    assert_eq!(warnings.len(), 1, "one line per kind of hole: {warnings:?}");
    let line = &warnings[0];
    assert!(
        line.len() <= MAX_WARNING_BYTES,
        "the warning is {} bytes long, over the cap of {MAX_WARNING_BYTES}: {}",
        line.len(),
        head(line)
    );
    assert!(
        line.contains(&format!("{HOLES} file(s) skipped")),
        "the warning publishes the exact count: {}",
        head(line)
    );
    assert!(
        line.contains("site.yml#0"),
        "the warning shows a sample of the holes it counted: {}",
        head(line)
    );
}

/// The warning about the refused relations is a summary of them.
#[test]
fn the_warning_about_the_refused_relations_is_a_summary_of_them() {
    let warnings = incomplete_warnings(&facts_with_rejected_relations());
    assert_eq!(warnings.len(), 1, "one line per kind of hole: {warnings:?}");
    let line = &warnings[0];
    assert!(
        line.len() <= MAX_WARNING_BYTES,
        "the warning is {} bytes long, over the cap of {MAX_WARNING_BYTES}: {}",
        line.len(),
        head(line)
    );
    assert!(
        line.contains(&format!("{HOLES} relation(s) refused by the repository")),
        "the warning publishes the exact count: {}",
        head(line)
    );
    assert!(
        line.contains("source_0 -> target_0"),
        "the warning shows a sample of the holes it counted: {}",
        head(line)
    );
}

/// A build that covered the tree says nothing.
#[test]
fn a_build_that_covered_the_tree_warns_about_nothing() {
    let facts = CoverageFacts::default();
    assert!(
        incomplete_warnings(&facts).is_empty(),
        "a complete build has no holes to warn about"
    );
}

/// A hole whose name does not fit is cut, and the line says that it was.
#[test]
fn a_hole_that_does_not_fit_is_cut_and_the_line_says_so() {
    let huge = "x".repeat(MAX_WARNING_BYTES * 2);
    let facts = CoverageFacts {
        skipped: Vec::new(),
        pending: vec![PendingReference {
            origin_file: huge.clone(),
            origin_line: Some(1),
            source: SymbolId::new("call_0"),
            target: huge,
            kind: DependencyType::References,
            class: PendingClass::Unresolved,
        }],
        rejected: Vec::new(),
    };
    let warnings = incomplete_warnings(&facts);
    let line = &warnings[0];
    assert!(
        line.len() <= MAX_WARNING_BYTES,
        "the warning is {} bytes long, over the cap of {MAX_WARNING_BYTES}: {}",
        line.len(),
        head(line)
    );
    assert!(
        line.contains("1 reference(s) without a declaration"),
        "the warning publishes the exact count even when it had to cut the hole: {}",
        head(line)
    );
    assert!(
        line.contains('…'),
        "the line says that the name was cut: {}",
        head(line)
    );
}

/// A build with few holes lists them all, and says nothing about the ones
/// it left out.
#[test]
fn a_build_with_few_holes_lists_them_all() {
    let facts = CoverageFacts {
        skipped: Vec::new(),
        pending: (0..3)
            .map(|index| PendingReference {
                origin_file: PATH.to_string(),
                origin_line: Some(index as u32),
                source: SymbolId::new(format!("call_{index}")),
                target: format!("ansible:role:playbook_{index}"),
                kind: DependencyType::References,
                class: PendingClass::Unresolved,
            })
            .collect(),
        rejected: Vec::new(),
    };
    let warnings = incomplete_warnings(&facts);
    let line = &warnings[0];
    assert!(
        !line.contains("more"),
        "nothing was left out, so the line does not say that something was: {}",
        head(line)
    );
    for index in 0..3 {
        assert!(
            line.contains(&format!("playbook_{index}")),
            "the line lists the hole {index}: {}",
            head(line)
        );
    }
}
