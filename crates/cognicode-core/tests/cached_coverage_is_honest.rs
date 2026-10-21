//! La cobertura dice lo mismo en caliente que en frío.
//!
//! La métrica de cobertura cuenta `parsed_files`: los ficheros que el
//! recorrido volvió a interpretar. Un fichero recuperado de la caché de
//! ficheros no cuenta. Así, la segunda construcción sobre el mismo árbol —
//! donde todo sale de la caché — informaba de **0 % de cobertura sobre un
//! grafo completo**, y la primera construcción informaba de un porcentaje
//! que tampoco era el de un grafo completo, porque el denominador contaba
//! también los ficheros que el grafo nunca iba a cubrir.
//!
//! El producto no puede publicar dos números distintos para el mismo árbol.
//! Y el número tiene que decir *qué* ficheros cuenta: los cubiertos, los
//! interpretados de nuevo y los reutilizados de la caché son tres hechos,
//! no uno.
//!
//! Este fichero fija ese contrato:
//!
//! - un build en caliente informa la misma cobertura que uno en frío;
//! - el informe separa cubiertos, reinterpretados y reutilizados;
//! - un fichero que no se pudo leer no cuenta como cubierto.
//!
//! El corpus es el real —`testdata/silent_errors_corpus`—, copiado a un
//! directorio temporal para poder dejar un fichero ilegible sin tocar el
//! repositorio.

use cognicode_core::application::dto::GraphCoverageMetrics;
use cognicode_core::application::services::analysis_service::AnalysisService;
use std::path::{Path, PathBuf};

/// El corpus versionado: tres ficheros Rust que se leen bien.
fn corpus_path() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set during cargo test");
    PathBuf::from(manifest).join("testdata/silent_errors_corpus")
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else if file_type.is_file() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Una copia limpia del corpus en un directorio temporal.
fn clean_corpus() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dst = tmp.path().join("corpus");
    copy_dir_recursive(&corpus_path(), &dst).expect("copy corpus");
    (tmp, dst)
}

fn build(root: &Path, service: &AnalysisService) -> GraphCoverageMetrics {
    service
        .build_project_graph(root)
        .expect("build_project_graph returns Ok even with skipped files");
    service
        .get_coverage_metrics()
        .expect("coverage metrics are populated after a build")
}

/// Un fichero que no se pudo leer no es un fichero cubierto.
#[test]
fn a_file_that_could_not_be_read_is_not_covered() {
    use std::os::unix::fs::PermissionsExt;

    let (_tmp, root) = clean_corpus();
    let unreadable = root.join("src").join("unreadable.rs");
    std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000))
        .expect("set_permissions 000");

    let service = AnalysisService::new();
    let coverage = build(&root, &service);

    assert_eq!(
        coverage.total_source_files, 3,
        "the three Rust files of the corpus are the files the graph is built from"
    );
    assert_eq!(
        coverage.covered_files, 2,
        "the file that could not be read is not covered; got coverage={coverage:?}"
    );
    assert!(
        coverage.coverage_percent < 100.0,
        "a file that could not be read leaves the coverage below 100 %; got {coverage:?}"
    );
}

/// El informe separa los ficheros cubiertos, los interpretados de nuevo y
/// los reutilizados de la caché.
#[test]
fn the_report_separates_reparsed_from_reused() {
    let (_tmp, root) = clean_corpus();
    let service = AnalysisService::new();

    let cold = build(&root, &service);
    assert_eq!(
        cold.parsed_files, 3,
        "a cold build interprets every file; got {cold:?}"
    );
    assert_eq!(
        cold.cached_files, 0,
        "a cold build reuses nothing; got {cold:?}"
    );
    assert_eq!(
        cold.covered_files, 3,
        "a cold build covers every file; got {cold:?}"
    );

    let warm = build(&root, &service);
    assert_eq!(
        warm.parsed_files, 0,
        "a warm build interprets nothing; got {warm:?}"
    );
    assert_eq!(
        warm.cached_files, 3,
        "a warm build reuses every file; got {warm:?}"
    );
    assert_eq!(
        warm.covered_files, 3,
        "a warm build covers the same files as a cold one; got {warm:?}"
    );
}

/// Un build en caliente sobre el mismo árbol informa la misma cobertura que
/// uno en frío. El árbol no ha cambiado: la cobertura tampoco.
#[test]
fn a_warm_build_reports_the_same_coverage_as_a_cold_one() {
    let (_tmp, root) = clean_corpus();
    let service = AnalysisService::new();

    let cold = build(&root, &service);
    let warm = build(&root, &service);

    assert_eq!(
        warm.covered_files, cold.covered_files,
        "the same tree covers the same files warm and cold; cold={cold:?} warm={warm:?}"
    );
    assert_eq!(
        warm.coverage_percent, cold.coverage_percent,
        "the same tree reports the same coverage warm and cold; cold={cold:?} warm={warm:?}"
    );
    assert_eq!(
        cold.coverage_percent, 100.0,
        "a corpus whose files are all readable is fully covered; got {cold:?}"
    );
}
