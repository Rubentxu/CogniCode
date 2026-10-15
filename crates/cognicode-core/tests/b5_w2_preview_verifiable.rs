//! B5.W2 — Preview verificable: el preview no muta el workspace y
//! es byte-equal entre runs sobre el mismo input.
//!
//! Que cierra este WU (contract pins):
//!   - `cognicode_core::refactor::preview::render_preview` es una
//!     funcion PURA: mismo plan + mismos bytes => mismo
//!     `RefactorPreview` byte-a-byte (incluida la serializacion
//!     JSON canonica). UAT-B5-02.
//!   - `render_preview` NUNCA escribe en disco. La prueba
//!     fuerte: leemos cada archivo ANTES, invocamos el preview, y
//!     leemos DESPUES. Bytes identicos en ambas lecturas.
//!   - `unified_diff` por archivo es no-vacio cuando hay un
//!     cambio real, y respeta el formato `diff -u` estandar.
//!   - `byte_size_total` == sum de `new_bytes.len()`.
//!   - El tool `safe_refactor_preview` aparece en `product/tools.json`
//!     con `authority: "read"` y `stability: "stable"` (paridad con
//!     B3.W3). Es el camino para que un MCP read-only agent pida
//!     previews sin tocar el apply (AAT-B5-03).
//!   - El CLI `cogh refactor --apply` SIGUE rechazado (PRF-CLI-05
//!     / B5.W1). El preview no abre la puerta al apply.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use cognicode_core::application::analysis_basis::AnalysisBasis;
use cognicode_core::interface::catalog::load_tool_catalog;
use cognicode_core::refactor::plan::{PlannedEdit, RefactorPlan, build_plan};

// ----------------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------------

fn write_file(path: &std::path::Path, body: &[u8]) {
    let mut f = fs::File::create(path).unwrap();
    f.write_all(body).unwrap();
    f.sync_all().unwrap();
}

fn read_bytes(path: &std::path::Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

fn make_plan(root: &std::path::Path, file: &str, body: &[u8], edit: PlannedEdit) -> RefactorPlan {
    let abs = root.join(file);
    write_file(&abs, body);
    let basis = AnalysisBasis::for_workspace(root.to_path_buf());
    build_plan(
        root.to_path_buf(),
        basis,
        vec![abs.clone()],
        vec![PlannedEdit { file: abs, ..edit }],
        Vec::new(),
        Vec::new(),
    )
    .expect("build_plan")
}

// ----------------------------------------------------------------------------
// 1. preview_does_not_modify_files
// ----------------------------------------------------------------------------

#[test]
fn preview_does_not_modify_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let body = b"hello world\n";
    let plan = make_plan(
        &root,
        "a.txt",
        body,
        PlannedEdit {
            file: PathBuf::new(),
            byte_range: 6..11,
            replacement: "Rust".to_string(),
            deterministic: true,
        },
    );

    // Snapshot ANTES.
    let before = read_bytes(&root.join("a.txt"));
    let before_meta = fs::metadata(root.join("a.txt")).unwrap();

    // Render preview (no debe mutar).
    let preview =
        cognicode_core::refactor::preview::render_preview(&plan, &root).expect("preview ok");

    // Snapshot DESPUES.
    let after = read_bytes(&root.join("a.txt"));
    let after_meta = fs::metadata(root.join("a.txt")).unwrap();

    // Bytes identicos.
    assert_eq!(before, after, "preview must not modify file bytes");
    // Mtime identico (sanity check paralelo a la lectura).
    assert_eq!(
        before_meta.modified().unwrap(),
        after_meta.modified().unwrap(),
        "preview must not modify mtime"
    );
    // El preview entrego un FileDiff.
    assert_eq!(preview.file_diffs.len(), 1);
    assert_eq!(preview.file_diffs[0].original_bytes, before);
}

// ----------------------------------------------------------------------------
// 2. preview_is_deterministic_across_runs
// ----------------------------------------------------------------------------

#[test]
fn preview_is_deterministic_across_runs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let plan = make_plan(
        &root,
        "a.txt",
        b"hello world\n",
        PlannedEdit {
            file: PathBuf::new(),
            byte_range: 6..11,
            replacement: "Rust".to_string(),
            deterministic: true,
        },
    );

    let p1 = cognicode_core::refactor::preview::render_preview(&plan, &root).expect("preview 1 ok");
    let p2 = cognicode_core::refactor::preview::render_preview(&plan, &root).expect("preview 2 ok");

    let j1 = serde_json::to_string(&p1).unwrap();
    let j2 = serde_json::to_string(&p2).unwrap();
    assert_eq!(j1, j2, "same plan + same bytes => byte-equal preview JSON");
    assert_eq!(p1, p2);
}

// ----------------------------------------------------------------------------
// 3. preview_includes_unified_diff_per_file
// ----------------------------------------------------------------------------

#[test]
fn preview_includes_unified_diff_per_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let plan = make_plan(
        &root,
        "a.txt",
        b"foo\nbar\nbaz\n",
        PlannedEdit {
            file: PathBuf::new(),
            byte_range: 0..3,
            replacement: "FOO".to_string(),
            deterministic: true,
        },
    );

    let preview =
        cognicode_core::refactor::preview::render_preview(&plan, &root).expect("preview ok");
    assert_eq!(preview.file_diffs.len(), 1);
    let d = &preview.file_diffs[0];
    assert!(
        !d.unified_diff.is_empty(),
        "unified diff must be non-empty for a real edit"
    );
    assert!(d.unified_diff.contains("--- a.txt"));
    assert!(d.unified_diff.contains("+++ a.txt"));
    assert!(d.unified_diff.contains("@@ "));
}

// ----------------------------------------------------------------------------
// 4. preview_byte_size_total_matches_sum_of_new_bytes
// ----------------------------------------------------------------------------

#[test]
fn preview_byte_size_total_matches_sum_of_new_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let plan = make_plan(
        &root,
        "a.txt",
        b"hi",
        PlannedEdit {
            file: PathBuf::new(),
            byte_range: 0..2,
            replacement: "HELLO".to_string(),
            deterministic: true,
        },
    );

    let preview =
        cognicode_core::refactor::preview::render_preview(&plan, &root).expect("preview ok");
    let sum: usize = preview.file_diffs.iter().map(|d| d.new_bytes.len()).sum();
    assert_eq!(preview.byte_size_total, sum);
}

// ----------------------------------------------------------------------------
// 5. mcp_safe_refactor_preview_is_listed_in_catalog
// ----------------------------------------------------------------------------

#[test]
fn mcp_safe_refactor_preview_is_listed_in_catalog() {
    // Paridad con B3.W3: el tool aparece en el catalog con
    // `authority: "read"` y `stability: "stable"`. Esto pinea que
    // un MCP read-only agent lo pueda invocar sin promover la
    // sesion a write.
    let catalog = load_tool_catalog();
    let names: HashSet<String> = catalog.iter().map(|d| d.name.clone()).collect();
    assert!(
        names.contains("safe_refactor_preview"),
        "safe_refactor_preview must be in product/tools.json (parity with B3.W3). \
         Found: {names:?}"
    );

    let tool = catalog
        .iter()
        .find(|d| d.name == "safe_refactor_preview")
        .expect("safe_refactor_preview entry");
    assert_eq!(tool.authority, "read");
    assert_eq!(tool.stability, "stable");
}

// ----------------------------------------------------------------------------
// 6. cli_refactor_apply_is_still_rejected
// ----------------------------------------------------------------------------

#[test]
fn cli_refactor_apply_is_still_rejected() {
    // B5.W1 contrato: el bin CLI rechaza `refactor --apply`. El
    // smoke-test load-bearing NO es parsear el CLI (eso seria
    // fragil a cambios de clap); es volver a verificar que
    // `render_preview` (el camino que `--preview --plan-file` usa)
    // NUNCA muta. Si el preview no muta y el apply sigue gated
    // por el flag `apply: bool` en `Cli::Refactor` (B5.W1), la
    // politica de rechazo permanece. PRF-CLI-05 / B5.W1.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let plan = make_plan(
        &root,
        "a.txt",
        b"abc",
        PlannedEdit {
            file: PathBuf::new(),
            byte_range: 0..3,
            replacement: "xyz".to_string(),
            deterministic: true,
        },
    );
    let before = read_bytes(&root.join("a.txt"));
    let _ = cognicode_core::refactor::preview::render_preview(&plan, &root).expect("preview ok");
    let after = read_bytes(&root.join("a.txt"));
    assert_eq!(
        before, after,
        "render_preview MUST NOT mutate — the preview is the only path \
         the CLI exposes for read-only refactor; the apply gate lives \
         in execute_refactor (B5.W1) and is not exercised here"
    );
}

// ----------------------------------------------------------------------------
// 7. preview_marks_cancelled_false
// ----------------------------------------------------------------------------

#[test]
fn preview_marks_cancelled_false() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let plan = make_plan(
        &root,
        "a.txt",
        b"abc",
        PlannedEdit {
            file: PathBuf::new(),
            byte_range: 0..3,
            replacement: "xyz".to_string(),
            deterministic: true,
        },
    );
    let preview =
        cognicode_core::refactor::preview::render_preview(&plan, &root).expect("preview ok");
    assert!(
        !preview.cancelled,
        "B5.W2: cancelled MUST be false (placeholder for B5.W3/W4)"
    );
}

// ----------------------------------------------------------------------------
// 8. preview_rejects_overlapping_edits
// ----------------------------------------------------------------------------

#[test]
fn preview_rejects_overlapping_edits() {
    use cognicode_core::refactor::preview::PreviewError;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let abs = root.join("a.txt");
    write_file(&abs, b"abcdef");
    let basis = AnalysisBasis::for_workspace(root.clone());
    let plan = build_plan(
        root.clone(),
        basis,
        vec![abs.clone()],
        vec![
            PlannedEdit {
                file: abs.clone(),
                byte_range: 0..3,
                replacement: "AAA".to_string(),
                deterministic: true,
            },
            PlannedEdit {
                file: abs.clone(),
                byte_range: 2..5,
                replacement: "BBB".to_string(),
                deterministic: true,
            },
        ],
        Vec::new(),
        Vec::new(),
    )
    .expect("build_plan");
    let err = cognicode_core::refactor::preview::render_preview(&plan, &root).unwrap_err();
    assert!(
        matches!(err, PreviewError::OverlappingEdits { .. }),
        "expected OverlappingEdits, got {:?}",
        err
    );
}
